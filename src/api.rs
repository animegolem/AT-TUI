use std::{
    fmt,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result, anyhow};
use chrono::{SecondsFormat, Utc};
use linkify::{LinkFinder, LinkKind};
use reqwest::{Client, Response, StatusCode};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::config::{Session, SessionStore};
use crate::model::PostRef;

const DEFAULT_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Session tokens live behind a shared lock so every clone handed to a
/// background task sees a refresh performed by any other clone. ATProto
/// rotates refresh tokens on use, so a clone refreshing privately would
/// strand all the others with revoked credentials.
#[derive(Debug, Clone)]
pub struct BskyClient {
    http: Client,
    store: SessionStore,
    session: Arc<RwLock<Session>>,
    refresh_gate: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedRecord {
    pub uri: String,
    pub cid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrpcError {
    status: StatusCode,
    code: Option<String>,
    message: Option<String>,
}

impl XrpcError {
    fn from_body(status: StatusCode, body: &str) -> Self {
        let value = serde_json::from_str::<Value>(body).ok();
        Self {
            status,
            code: value
                .as_ref()
                .and_then(|value| value.get("error"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            message: value
                .as_ref()
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        }
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    fn requires_session_refresh(&self) -> bool {
        self.code() == Some("ExpiredToken") || self.status == StatusCode::UNAUTHORIZED
    }
}

impl fmt::Display for XrpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Bluesky API returned {}", self.status)?;
        if let Some(code) = self.code() {
            write!(formatter, " ({code})")?;
        }
        if let Some(message) = self.message() {
            write!(formatter, ": {message}")?;
        }
        Ok(())
    }
}

impl std::error::Error for XrpcError {}

enum AuthenticatedRequest<'a> {
    Get {
        endpoint: &'a str,
        query: &'a [(String, String)],
    },
    Post {
        endpoint: &'a str,
        body: &'a Value,
    },
}

struct AuthenticatedResponse {
    response: Response,
    access_jwt: String,
}

impl AuthenticatedRequest<'_> {
    fn endpoint(&self) -> &str {
        match self {
            Self::Get { endpoint, .. } | Self::Post { endpoint, .. } => endpoint,
        }
    }
}

impl BskyClient {
    pub fn new(session: Session, store: SessionStore) -> Self {
        Self::with_http_timeout(session, store, DEFAULT_HTTP_TIMEOUT)
    }

    fn with_http_timeout(
        session: Session,
        store: SessionStore,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            http: Client::builder()
                .timeout(timeout)
                .build()
                .expect("valid Bluesky HTTP client configuration"),
            store,
            session: Arc::new(RwLock::new(session)),
            refresh_gate: Arc::new(Mutex::new(())),
        }
    }

    pub async fn login(
        service: &str,
        identifier: &str,
        app_password: &str,
        store: &SessionStore,
    ) -> Result<Session> {
        let session = Self::login_session(service, identifier, app_password).await?;
        store.save(&session)?;
        Ok(session)
    }

    pub async fn login_session(
        service: &str,
        identifier: &str,
        app_password: &str,
    ) -> Result<Session> {
        let http = Client::builder()
            .timeout(DEFAULT_HTTP_TIMEOUT)
            .build()
            .expect("valid Bluesky HTTP client configuration");
        let url = xrpc_url(service, "com.atproto.server.createSession");
        let response = http
            .post(url)
            .json(&json!({
                "identifier": identifier,
                "password": app_password,
            }))
            .send()
            .await
            .context("could not create Bluesky session")?;

        let value = response_json(response).await?;
        let session = Session {
            service: service.trim_end_matches('/').to_owned(),
            handle: required_string(&value, "handle")?,
            did: required_string(&value, "did")?,
            access_jwt: required_string(&value, "accessJwt")?,
            refresh_jwt: required_string(&value, "refreshJwt")?,
        };
        Ok(session)
    }

    pub fn session(&self) -> Session {
        self.session.read().expect("session lock poisoned").clone()
    }

    fn access_jwt(&self) -> String {
        self.session
            .read()
            .expect("session lock poisoned")
            .access_jwt
            .clone()
    }

    #[cfg(test)]
    fn set_tokens_for_test(&self, access_jwt: &str, refresh_jwt: &str) {
        let mut session = self.session.write().expect("session lock poisoned");
        session.access_jwt = access_jwt.to_owned();
        session.refresh_jwt = refresh_jwt.to_owned();
    }

    pub fn store(&self) -> SessionStore {
        self.store.clone()
    }

    pub async fn get_timeline(&mut self, cursor: Option<&str>, limit: u16) -> Result<Value> {
        let query = timeline_query(cursor, limit);
        self.get("app.bsky.feed.getTimeline", &query).await
    }

    pub async fn get_feed(
        &mut self,
        feed: &str,
        cursor: Option<&str>,
        limit: u16,
    ) -> Result<Value> {
        let query = feed_query(feed, cursor, limit);
        self.get("app.bsky.feed.getFeed", &query).await
    }

    pub async fn get_author_feed(
        &mut self,
        actor: &str,
        cursor: Option<&str>,
        limit: u16,
    ) -> Result<Value> {
        let query = author_feed_query(actor, cursor, limit);
        self.get("app.bsky.feed.getAuthorFeed", &query).await
    }

    pub async fn get_profile(&mut self, actor: &str) -> Result<Value> {
        let query = actor_query(actor);
        self.get("app.bsky.actor.getProfile", &query).await
    }

    pub async fn get_preferences(&mut self) -> Result<Value> {
        let query: Vec<(String, String)> = Vec::new();
        self.get("app.bsky.actor.getPreferences", &query).await
    }

    pub async fn get_post_thread(&mut self, uri: &str) -> Result<Value> {
        let query = post_thread_query(uri);
        self.get("app.bsky.feed.getPostThread", &query).await
    }

    pub async fn get_unread_notification_count(&mut self) -> Result<u64> {
        let query: Vec<(String, String)> = Vec::new();
        let root = self
            .get("app.bsky.notification.getUnreadCount", &query)
            .await?;
        unread_notification_count(&root)
    }

    pub async fn list_notifications(&mut self, cursor: Option<&str>, limit: u16) -> Result<Value> {
        let query = notification_query(cursor, limit);
        self.get("app.bsky.notification.listNotifications", &query)
            .await
    }

    pub async fn update_seen(&mut self, seen_at: &str) -> Result<()> {
        self.post_empty(
            "app.bsky.notification.updateSeen",
            json!({ "seenAt": seen_at }),
        )
        .await
    }

    pub async fn refresh_session(&mut self) -> Result<()> {
        let observed_access = self.access_jwt();
        self.refresh_session_if_unchanged(&observed_access).await
    }

    async fn refresh_session_if_unchanged(&mut self, observed_access: &str) -> Result<()> {
        let gate = self.refresh_gate.clone();
        let _guard = gate.lock().await;
        if self.access_jwt() != observed_access {
            // Another task refreshed while we waited on the gate; the
            // rotated refresh token must not be used a second time.
            return Ok(());
        }

        let (service, refresh_jwt) = {
            let session = self.session.read().expect("session lock poisoned");
            (session.service.clone(), session.refresh_jwt.clone())
        };
        let response = self
            .http
            .post(xrpc_url(&service, "com.atproto.server.refreshSession"))
            .bearer_auth(&refresh_jwt)
            .send()
            .await
            .context("could not refresh Bluesky session")?;

        let value = response_json(response).await?;
        let access_jwt = required_string(&value, "accessJwt")?;
        let refresh_jwt = required_string(&value, "refreshJwt")?;
        let handle = value.get("handle").and_then(Value::as_str);
        let did = value.get("did").and_then(Value::as_str);
        let updated = {
            let mut session = self.session.write().expect("session lock poisoned");
            session.access_jwt = access_jwt;
            session.refresh_jwt = refresh_jwt;
            if let Some(handle) = handle {
                session.handle = handle.to_owned();
            }
            if let Some(did) = did {
                session.did = did.to_owned();
            }
            session.clone()
        };
        self.store.save(&updated)?;
        Ok(())
    }

    pub async fn create_like(&mut self, subject: &PostRef) -> Result<CreatedRecord> {
        let record = like_record_json(subject);
        self.create_record("app.bsky.feed.like", record).await
    }

    pub async fn create_repost(&mut self, subject: &PostRef) -> Result<CreatedRecord> {
        let record = repost_record_json(subject);
        self.create_record("app.bsky.feed.repost", record).await
    }

    pub async fn create_follow(&mut self, subject_did: &str) -> Result<CreatedRecord> {
        let record = follow_record_json(subject_did);
        self.create_record("app.bsky.graph.follow", record).await
    }

    pub async fn resolve_handle(&mut self, handle: &str) -> Result<String> {
        let query = vec![("handle".to_owned(), handle.to_owned())];
        let root = self
            .get("com.atproto.identity.resolveHandle", &query)
            .await?;
        required_string(&root, "did")
    }

    pub async fn create_post(
        &mut self,
        text: &str,
        reply: Option<(PostRef, PostRef)>,
        quote: Option<PostRef>,
    ) -> Result<CreatedRecord> {
        let mut facets = link_facets(text);
        for mention in mention_candidates(text) {
            // Unresolvable mentions post as plain text rather than failing.
            if let Ok(did) = self.resolve_handle(&mention.handle).await {
                facets.push(mention_facet(&mention, &did));
            }
        }
        let record = post_record_json(text, reply, quote, facets);
        self.create_record("app.bsky.feed.post", record).await
    }

    pub async fn delete_record_uri(&mut self, record_uri: &str) -> Result<()> {
        let record = at_uri_parts(record_uri)?;
        let body = json!({
            "repo": record.repo,
            "collection": record.collection,
            "rkey": record.rkey,
        });
        self.post_empty("com.atproto.repo.deleteRecord", body).await
    }

    async fn create_record(&mut self, collection: &str, record: Value) -> Result<CreatedRecord> {
        let body = json!({
            "repo": self.session().did,
            "collection": collection,
            "record": record,
        });
        let value = self
            .post_json("com.atproto.repo.createRecord", body)
            .await?;
        Ok(CreatedRecord {
            uri: required_string(&value, "uri")?,
            cid: required_string(&value, "cid")?,
        })
    }

    async fn get(&mut self, endpoint: &str, query: &[(String, String)]) -> Result<Value> {
        self.authenticated_json(AuthenticatedRequest::Get { endpoint, query })
            .await
    }

    async fn post_json(&mut self, endpoint: &str, body: Value) -> Result<Value> {
        self.authenticated_json(AuthenticatedRequest::Post {
            endpoint,
            body: &body,
        })
        .await
    }

    async fn post_empty(&mut self, endpoint: &str, body: Value) -> Result<()> {
        self.post_json(endpoint, body).await.map(|_| ())
    }

    async fn authenticated_json(&mut self, request: AuthenticatedRequest<'_>) -> Result<Value> {
        let AuthenticatedResponse {
            response,
            access_jwt,
        } = self.send_authenticated(&request).await?;
        match response_json(response).await {
            Err(error)
                if error
                    .downcast_ref::<XrpcError>()
                    .is_some_and(XrpcError::requires_session_refresh) =>
            {
                self.refresh_session_if_unchanged(&access_jwt)
                    .await
                    .context("could not refresh expired Bluesky session")?;
                let retry = self.send_authenticated(&request).await?;
                response_json(retry.response).await
            }
            result => result,
        }
    }

    async fn send_authenticated(
        &self,
        request: &AuthenticatedRequest<'_>,
    ) -> Result<AuthenticatedResponse> {
        let (service, access_jwt) = {
            let session = self.session.read().expect("session lock poisoned");
            (session.service.clone(), session.access_jwt.clone())
        };
        let request_builder = match request {
            AuthenticatedRequest::Get { endpoint, query } => {
                self.http.get(xrpc_url(&service, endpoint)).query(*query)
            }
            AuthenticatedRequest::Post { endpoint, body } => {
                self.http.post(xrpc_url(&service, endpoint)).json(*body)
            }
        };
        let response = request_builder
            .bearer_auth(&access_jwt)
            .send()
            .await
            .with_context(|| format!("could not call {}", request.endpoint()))?;
        Ok(AuthenticatedResponse {
            response,
            access_jwt,
        })
    }
}

fn xrpc_url(service: &str, endpoint: &str) -> String {
    format!("{}/xrpc/{endpoint}", service.trim_end_matches('/'))
}

fn timeline_query(cursor: Option<&str>, limit: u16) -> Vec<(String, String)> {
    let mut query = vec![("limit".to_owned(), limit.to_string())];
    if let Some(cursor) = cursor {
        query.push(("cursor".to_owned(), cursor.to_owned()));
    }
    query
}

fn feed_query(feed: &str, cursor: Option<&str>, limit: u16) -> Vec<(String, String)> {
    let mut query = vec![
        ("feed".to_owned(), feed.to_owned()),
        ("limit".to_owned(), limit.to_string()),
    ];
    if let Some(cursor) = cursor {
        query.push(("cursor".to_owned(), cursor.to_owned()));
    }
    query
}

fn author_feed_query(actor: &str, cursor: Option<&str>, limit: u16) -> Vec<(String, String)> {
    let mut query = vec![
        ("actor".to_owned(), actor.to_owned()),
        ("filter".to_owned(), "posts_with_replies".to_owned()),
        ("limit".to_owned(), limit.to_string()),
    ];
    if let Some(cursor) = cursor {
        query.push(("cursor".to_owned(), cursor.to_owned()));
    }
    query
}

fn actor_query(actor: &str) -> Vec<(String, String)> {
    vec![("actor".to_owned(), actor.to_owned())]
}

fn notification_query(cursor: Option<&str>, limit: u16) -> Vec<(String, String)> {
    let mut query = vec![("limit".to_owned(), limit.to_string())];
    if let Some(cursor) = cursor {
        query.push(("cursor".to_owned(), cursor.to_owned()));
    }
    query
}

fn post_thread_query(uri: &str) -> Vec<(String, String)> {
    vec![
        ("uri".to_owned(), uri.to_owned()),
        ("depth".to_owned(), "8".to_owned()),
        ("parentHeight".to_owned(), "80".to_owned()),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AtUriParts {
    repo: String,
    collection: String,
    rkey: String,
}

fn at_uri_parts(uri: &str) -> Result<AtUriParts> {
    let path = uri
        .strip_prefix("at://")
        .ok_or_else(|| anyhow!("not an at:// URI: {uri}"))?;
    let mut parts = path.splitn(3, '/');
    let repo = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("AT URI is missing repo: {uri}"))?;
    let collection = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("AT URI is missing collection: {uri}"))?;
    let rkey = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("AT URI is missing record key: {uri}"))?;
    Ok(AtUriParts {
        repo: repo.to_owned(),
        collection: collection.to_owned(),
        rkey: rkey.to_owned(),
    })
}

fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn like_record_json(subject: &PostRef) -> Value {
    json!({
        "$type": "app.bsky.feed.like",
        "subject": {
            "uri": subject.uri.clone(),
            "cid": subject.cid.clone(),
        },
        "createdAt": now_timestamp(),
    })
}

fn repost_record_json(subject: &PostRef) -> Value {
    json!({
        "$type": "app.bsky.feed.repost",
        "subject": {
            "uri": subject.uri.clone(),
            "cid": subject.cid.clone(),
        },
        "createdAt": now_timestamp(),
    })
}

fn follow_record_json(subject_did: &str) -> Value {
    json!({
        "$type": "app.bsky.graph.follow",
        "subject": subject_did,
        "createdAt": now_timestamp(),
    })
}

fn post_record_json(
    text: &str,
    reply: Option<(PostRef, PostRef)>,
    quote: Option<PostRef>,
    facets: Vec<Value>,
) -> Value {
    let mut record = json!({
        "$type": "app.bsky.feed.post",
        "text": text,
        "createdAt": now_timestamp(),
    });

    if !facets.is_empty() {
        record["facets"] = Value::Array(facets);
    }

    if let Some((root, parent)) = reply {
        record["reply"] = json!({
            "root": {"uri": root.uri, "cid": root.cid},
            "parent": {"uri": parent.uri, "cid": parent.cid},
        });
    }

    if let Some(quote) = quote {
        record["embed"] = json!({
            "$type": "app.bsky.embed.record",
            "record": {"uri": quote.uri, "cid": quote.cid},
        });
    }

    record
}

// Facet index fields are UTF-8 byte offsets, which is exactly what linkify
// and our byte scanner produce; no char-index conversion is needed.
fn link_facets(text: &str) -> Vec<Value> {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url]);
    finder
        .links(text)
        .map(|link| {
            json!({
                "index": {"byteStart": link.start(), "byteEnd": link.end()},
                "features": [{
                    "$type": "app.bsky.richtext.facet#link",
                    "uri": link.as_str(),
                }],
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MentionCandidate {
    byte_start: usize,
    byte_end: usize,
    handle: String,
}

fn mention_facet(mention: &MentionCandidate, did: &str) -> Value {
    json!({
        "index": {"byteStart": mention.byte_start, "byteEnd": mention.byte_end},
        "features": [{
            "$type": "app.bsky.richtext.facet#mention",
            "did": did,
        }],
    })
}

fn mention_candidates(text: &str) -> Vec<MentionCandidate> {
    let bytes = text.as_bytes();
    let mut candidates = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'@' && (index == 0 || !is_handle_byte(bytes[index - 1])) {
            let mut end = index + 1;
            while end < bytes.len() && is_handle_byte(bytes[end]) {
                end += 1;
            }
            // A sentence-ending dot is punctuation, not part of the handle.
            while end > index + 1 && bytes[end - 1] == b'.' {
                end -= 1;
            }
            let handle = &text[index + 1..end];
            if is_plausible_handle(handle) {
                candidates.push(MentionCandidate {
                    byte_start: index,
                    byte_end: end,
                    handle: handle.to_owned(),
                });
            }
            index = end.max(index + 1);
        } else {
            index += 1;
        }
    }
    candidates
}

fn is_handle_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-'
}

fn is_plausible_handle(handle: &str) -> bool {
    let segments: Vec<&str> = handle.split('.').collect();
    if segments.len() < 2 {
        return false;
    }
    let tld = segments.last().expect("at least two segments");
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    segments.iter().all(|segment| {
        !segment.is_empty()
            && !segment.starts_with('-')
            && !segment.ends_with('-')
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

async fn response_json(response: Response) -> Result<Value> {
    let status = response.status();
    let text = response
        .text()
        .await
        .context("could not read response body")?;
    if !status.is_success() {
        return Err(XrpcError::from_body(status, &text).into());
    }
    serde_json::from_str(&text).context("could not parse Bluesky API response")
}

fn required_string(value: &Value, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("Bluesky response did not include {field}"))
}

fn unread_notification_count(value: &Value) -> Result<u64> {
    value
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("Bluesky response did not include notification count"))
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread::{self, JoinHandle},
        time::Duration,
    };

    use super::*;

    struct ExpectedRequest {
        method: &'static str,
        path: &'static str,
        authorization: &'static str,
        status: u16,
        body: &'static str,
    }

    fn spawn_xrpc_server(expectations: Vec<ExpectedRequest>) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            for expected in expectations {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let request = read_http_request(&mut stream);
                let expected_request_line =
                    format!("{} {} HTTP/1.1", expected.method, expected.path);
                let expected_authorization =
                    format!("authorization: Bearer {}", expected.authorization);
                let request_matches = request
                    .lines()
                    .next()
                    .is_some_and(|line| line == expected_request_line)
                    && request
                        .lines()
                        .any(|line| line.eq_ignore_ascii_case(&expected_authorization));

                write_http_response(&mut stream, expected.status, expected.body);
                assert!(
                    request_matches,
                    "unexpected request; wanted {expected_request_line} with {expected_authorization}, got:\n{request}"
                );
            }
        });
        (format!("http://{address}"), handle)
    }

    fn spawn_stalled_server(delay: Duration) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let _ = read_http_request(&mut stream);
            thread::sleep(delay);
        });
        (format!("http://{address}"), handle)
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let mut expected_len = None;

        loop {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);

            if expected_len.is_none()
                && let Some(header_end) = find_bytes(&request, b"\r\n\r\n")
            {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_len = headers
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(name, value)| {
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                    })
                    .unwrap_or(0);
                expected_len = Some(header_end + 4 + content_len);
            }

            if expected_len.is_some_and(|expected| request.len() >= expected) {
                break;
            }
        }

        String::from_utf8(request).unwrap()
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn write_http_response(stream: &mut TcpStream, status: u16, body: &str) {
        let reason = match status {
            200 => "OK",
            400 => "Bad Request",
            401 => "Unauthorized",
            500 => "Internal Server Error",
            _ => "Test Response",
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    }

    fn test_session(service: String) -> Session {
        Session {
            service,
            handle: "alice.test".into(),
            did: "did:plc:alice".into(),
            access_jwt: "old-access".into(),
            refresh_jwt: "old-refresh".into(),
        }
    }

    #[test]
    fn builds_xrpc_url_without_double_slash() {
        assert_eq!(
            xrpc_url("https://bsky.social/", "app.bsky.feed.getTimeline"),
            "https://bsky.social/xrpc/app.bsky.feed.getTimeline"
        );
    }

    #[test]
    fn requires_session_fields() {
        let value = json!({"handle": "alice.test"});
        assert!(required_string(&value, "accessJwt").is_err());
    }

    #[test]
    fn post_thread_query_requests_parent_chain() {
        let query = post_thread_query("at://did:plc:alice/app.bsky.feed.post/1");
        assert!(query.contains(&("depth".into(), "8".into())));
        assert!(query.contains(&("parentHeight".into(), "80".into())));
    }

    #[test]
    fn builds_home_timeline_query() {
        let query = timeline_query(Some("cursor"), 50);
        assert_eq!(
            query,
            vec![
                ("limit".into(), "50".into()),
                ("cursor".into(), "cursor".into())
            ]
        );
    }

    #[test]
    fn builds_saved_feed_query() {
        let query = feed_query(
            "at://did:plc:alice/app.bsky.feed.generator/news",
            Some("cursor"),
            25,
        );
        assert_eq!(
            query,
            vec![
                (
                    "feed".into(),
                    "at://did:plc:alice/app.bsky.feed.generator/news".into()
                ),
                ("limit".into(), "25".into()),
                ("cursor".into(), "cursor".into())
            ]
        );
    }

    #[test]
    fn builds_author_feed_query() {
        let query = author_feed_query("did:plc:alice", Some("cursor"), 25);
        assert_eq!(
            query,
            vec![
                ("actor".into(), "did:plc:alice".into()),
                ("filter".into(), "posts_with_replies".into()),
                ("limit".into(), "25".into()),
                ("cursor".into(), "cursor".into())
            ]
        );
    }

    #[test]
    fn builds_profile_and_notification_queries() {
        assert_eq!(
            actor_query("alice.test"),
            vec![("actor".into(), "alice.test".into())]
        );
        assert_eq!(
            notification_query(Some("cursor"), 25),
            vec![
                ("limit".into(), "25".into()),
                ("cursor".into(), "cursor".into())
            ]
        );
    }

    #[test]
    fn parses_record_at_uri_parts() {
        let parts = at_uri_parts("at://did:plc:alice/app.bsky.feed.like/3jz").unwrap();
        assert_eq!(
            parts,
            AtUriParts {
                repo: "did:plc:alice".into(),
                collection: "app.bsky.feed.like".into(),
                rkey: "3jz".into()
            }
        );
        assert!(at_uri_parts("https://example.com").is_err());
    }

    #[test]
    fn builds_write_records() {
        let subject = PostRef {
            uri: "at://did:plc:bob/app.bsky.feed.post/1".into(),
            cid: "postcid".into(),
        };
        let like = like_record_json(&subject);
        assert_eq!(like["$type"], "app.bsky.feed.like");
        assert_eq!(like["subject"]["uri"].as_str(), Some(subject.uri.as_str()));

        let follow = follow_record_json("did:plc:alice");
        assert_eq!(follow["$type"], "app.bsky.graph.follow");
        assert_eq!(follow["subject"].as_str(), Some("did:plc:alice"));
        assert!(follow["createdAt"].as_str().is_some());

        let reply = post_record_json(
            "reply text",
            Some((
                PostRef {
                    uri: "root".into(),
                    cid: "rootcid".into(),
                },
                subject.clone(),
            )),
            None,
            Vec::new(),
        );
        assert_eq!(reply["reply"]["root"]["cid"], "rootcid");
        assert_eq!(reply["reply"]["parent"]["cid"], "postcid");
        assert!(reply.get("facets").is_none());

        let quote = post_record_json("quote text", None, Some(subject), Vec::new());
        assert_eq!(quote["embed"]["$type"], "app.bsky.embed.record");
        assert_eq!(quote["embed"]["record"]["cid"], "postcid");
    }

    #[test]
    fn link_facets_use_byte_offsets_after_multibyte_text() {
        let text = "🦋🦋 https://example.com done";
        let facets = link_facets(text);

        assert_eq!(facets.len(), 1);
        let start = facets[0]["index"]["byteStart"].as_u64().unwrap() as usize;
        let end = facets[0]["index"]["byteEnd"].as_u64().unwrap() as usize;
        assert_eq!(&text[start..end], "https://example.com");
        assert_eq!(
            facets[0]["features"][0]["uri"].as_str(),
            Some("https://example.com")
        );
    }

    #[test]
    fn mention_candidates_find_handles_and_skip_noise() {
        let text = "hi @alice.test and @bob.example.com. not@this.one @x @-bad.com";
        let candidates = mention_candidates(text);

        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.handle.as_str())
                .collect::<Vec<_>>(),
            vec!["alice.test", "bob.example.com"]
        );
        for candidate in &candidates {
            assert_eq!(
                &text[candidate.byte_start..candidate.byte_end],
                format!("@{}", candidate.handle)
            );
        }
    }

    #[test]
    fn post_record_includes_facets_when_present() {
        let text = "see https://example.com";
        let record = post_record_json(text, None, None, link_facets(text));

        assert_eq!(
            record["facets"][0]["features"][0]["$type"],
            "app.bsky.richtext.facet#link"
        );
    }

    #[test]
    fn clones_share_one_live_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::config::SessionStore::from_path(dir.path().join("accounts.json"));
        let session = Session {
            service: "https://bsky.social".into(),
            handle: "alice.test".into(),
            did: "did:plc:alice".into(),
            access_jwt: "old-access".into(),
            refresh_jwt: "old-refresh".into(),
        };
        let client = BskyClient::new(session, store);
        let clone = client.clone();

        clone.set_tokens_for_test("new-access", "new-refresh");

        assert_eq!(client.session().access_jwt, "new-access");
        assert_eq!(client.session().refresh_jwt, "new-refresh");
    }

    #[tokio::test]
    async fn refresh_skips_when_another_task_already_refreshed() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::config::SessionStore::from_path(dir.path().join("accounts.json"));
        let session = Session {
            service: "https://bsky.social".into(),
            handle: "alice.test".into(),
            did: "did:plc:alice".into(),
            access_jwt: "old-access".into(),
            refresh_jwt: "old-refresh".into(),
        };
        let client = BskyClient::new(session, store);

        // Hold the gate as a concurrent refresher would, rotate the tokens,
        // then release: the waiting refresh must observe the change and
        // return without spending the refresh token (no HTTP happens).
        let gate = client.refresh_gate.clone();
        let guard = gate.lock().await;
        let concurrent = client.clone();
        let waiting = tokio::spawn(async move {
            let mut concurrent = concurrent;
            concurrent.refresh_session().await
        });
        tokio::task::yield_now().await;
        client.set_tokens_for_test("new-access", "new-refresh");
        drop(guard);

        waiting.await.unwrap().unwrap();
        assert_eq!(client.session().access_jwt, "new-access");
    }

    #[tokio::test]
    async fn expired_token_400_refreshes_and_retries_get() {
        let (service, server) = spawn_xrpc_server(vec![
            ExpectedRequest {
                method: "GET",
                path: "/xrpc/app.bsky.feed.getTimeline?limit=1",
                authorization: "old-access",
                status: 400,
                body: r#"{"error":"ExpiredToken","message":"Token has expired"}"#,
            },
            ExpectedRequest {
                method: "POST",
                path: "/xrpc/com.atproto.server.refreshSession",
                authorization: "old-refresh",
                status: 200,
                body: r#"{"accessJwt":"new-access","refreshJwt":"new-refresh"}"#,
            },
            ExpectedRequest {
                method: "GET",
                path: "/xrpc/app.bsky.feed.getTimeline?limit=1",
                authorization: "new-access",
                status: 200,
                body: r#"{"feed":[]}"#,
            },
        ]);
        let dir = tempfile::tempdir().unwrap();
        let store = crate::config::SessionStore::from_path(dir.path().join("accounts.json"));
        let mut client = BskyClient::new(test_session(service), store.clone());
        let clone = client.clone();

        let root = client.get_timeline(None, 1).await.unwrap();

        server.join().unwrap();
        assert_eq!(root, json!({"feed": []}));
        assert_eq!(client.session().access_jwt, "new-access");
        assert_eq!(clone.session().refresh_jwt, "new-refresh");
        assert_eq!(store.load().unwrap().access_jwt, "new-access");
    }

    #[tokio::test]
    async fn unrelated_400_does_not_refresh_post() {
        let (service, server) = spawn_xrpc_server(vec![ExpectedRequest {
            method: "POST",
            path: "/xrpc/com.atproto.repo.createRecord",
            authorization: "old-access",
            status: 400,
            body: r#"{"error":"InvalidRequest","message":"Bad record"}"#,
        }]);
        let dir = tempfile::tempdir().unwrap();
        let store = crate::config::SessionStore::from_path(dir.path().join("accounts.json"));
        let mut client = BskyClient::new(test_session(service), store);
        let subject = PostRef {
            uri: "at://did:plc:bob/app.bsky.feed.post/1".into(),
            cid: "postcid".into(),
        };

        let error = client.create_like(&subject).await.unwrap_err();

        server.join().unwrap();
        let xrpc = error.downcast_ref::<XrpcError>().unwrap();
        assert_eq!(xrpc.status(), StatusCode::BAD_REQUEST);
        assert_eq!(xrpc.code(), Some("InvalidRequest"));
        assert_eq!(xrpc.message(), Some("Bad record"));
        assert_eq!(client.session().refresh_jwt, "old-refresh");
    }

    #[tokio::test]
    async fn failed_refresh_returns_typed_cause_without_retrying_request() {
        let (service, server) = spawn_xrpc_server(vec![
            ExpectedRequest {
                method: "GET",
                path: "/xrpc/app.bsky.feed.getTimeline?limit=1",
                authorization: "old-access",
                status: 400,
                body: r#"{"error":"ExpiredToken","message":"Token has expired"}"#,
            },
            ExpectedRequest {
                method: "POST",
                path: "/xrpc/com.atproto.server.refreshSession",
                authorization: "old-refresh",
                status: 400,
                body: r#"{"error":"InvalidToken","message":"Refresh token rejected"}"#,
            },
        ]);
        let dir = tempfile::tempdir().unwrap();
        let store = crate::config::SessionStore::from_path(dir.path().join("accounts.json"));
        let mut client = BskyClient::new(test_session(service), store);

        let error = client.get_timeline(None, 1).await.unwrap_err();

        server.join().unwrap();
        let xrpc = error.downcast_ref::<XrpcError>().unwrap();
        assert_eq!(xrpc.code(), Some("InvalidToken"));
        assert_eq!(client.session().access_jwt, "old-access");
    }

    #[tokio::test]
    async fn failed_retry_returns_typed_cause_without_refresh_loop() {
        let (service, server) = spawn_xrpc_server(vec![
            ExpectedRequest {
                method: "GET",
                path: "/xrpc/app.bsky.feed.getTimeline?limit=1",
                authorization: "old-access",
                status: 400,
                body: r#"{"error":"ExpiredToken","message":"Token has expired"}"#,
            },
            ExpectedRequest {
                method: "POST",
                path: "/xrpc/com.atproto.server.refreshSession",
                authorization: "old-refresh",
                status: 200,
                body: r#"{"accessJwt":"new-access","refreshJwt":"new-refresh"}"#,
            },
            ExpectedRequest {
                method: "GET",
                path: "/xrpc/app.bsky.feed.getTimeline?limit=1",
                authorization: "new-access",
                status: 500,
                body: r#"{"error":"UpstreamFailure","message":"Try later"}"#,
            },
        ]);
        let dir = tempfile::tempdir().unwrap();
        let store = crate::config::SessionStore::from_path(dir.path().join("accounts.json"));
        let mut client = BskyClient::new(test_session(service), store);

        let error = client.get_timeline(None, 1).await.unwrap_err();

        server.join().unwrap();
        let xrpc = error.downcast_ref::<XrpcError>().unwrap();
        assert_eq!(xrpc.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(xrpc.code(), Some("UpstreamFailure"));
        assert_eq!(client.session().access_jwt, "new-access");
    }

    #[tokio::test]
    async fn api_request_respects_configured_deadline() {
        let (service, server) = spawn_stalled_server(Duration::from_millis(100));
        let dir = tempfile::tempdir().unwrap();
        let store = crate::config::SessionStore::from_path(dir.path().join("accounts.json"));
        let mut client =
            BskyClient::with_http_timeout(test_session(service), store, Duration::from_millis(20));

        let error = client.get_timeline(None, 1).await.unwrap_err();

        server.join().unwrap();
        assert!(format!("{error:#}").contains("operation timed out"));
    }

    #[test]
    fn parses_unread_notification_count() {
        assert_eq!(unread_notification_count(&json!({"count": 4})).unwrap(), 4);
        assert!(unread_notification_count(&json!({})).is_err());
    }
}
