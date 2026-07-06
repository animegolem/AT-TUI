use std::sync::{Arc, RwLock};

use anyhow::{Context, Result, anyhow};
use chrono::{SecondsFormat, Utc};
use reqwest::{Client, Response, StatusCode};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::config::{Session, SessionStore};
use crate::model::PostRef;

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

impl BskyClient {
    pub fn new(session: Session, store: SessionStore) -> Self {
        Self {
            http: Client::new(),
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
        let http = Client::new();
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

    pub async fn create_post(
        &mut self,
        text: &str,
        reply: Option<(PostRef, PostRef)>,
        quote: Option<PostRef>,
    ) -> Result<CreatedRecord> {
        let record = post_record_json(text, reply, quote);
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
        let response = self.send_get(endpoint, query).await?;
        if response.status() == StatusCode::UNAUTHORIZED {
            self.refresh_session().await?;
            let retry = self.send_get(endpoint, query).await?;
            return response_json(retry).await;
        }
        response_json(response).await
    }

    async fn post_json(&mut self, endpoint: &str, body: Value) -> Result<Value> {
        let response = self.send_post(endpoint, &body).await?;
        if response.status() == StatusCode::UNAUTHORIZED {
            self.refresh_session().await?;
            let retry = self.send_post(endpoint, &body).await?;
            return response_json(retry).await;
        }
        response_json(response).await
    }

    async fn post_empty(&mut self, endpoint: &str, body: Value) -> Result<()> {
        self.post_json(endpoint, body).await.map(|_| ())
    }

    async fn send_get(&self, endpoint: &str, query: &[(String, String)]) -> Result<Response> {
        let (service, access_jwt) = {
            let session = self.session.read().expect("session lock poisoned");
            (session.service.clone(), session.access_jwt.clone())
        };
        self.http
            .get(xrpc_url(&service, endpoint))
            .bearer_auth(access_jwt)
            .query(query)
            .send()
            .await
            .with_context(|| format!("could not call {endpoint}"))
    }

    async fn send_post(&self, endpoint: &str, body: &Value) -> Result<Response> {
        let (service, access_jwt) = {
            let session = self.session.read().expect("session lock poisoned");
            (session.service.clone(), session.access_jwt.clone())
        };
        self.http
            .post(xrpc_url(&service, endpoint))
            .bearer_auth(access_jwt)
            .json(body)
            .send()
            .await
            .with_context(|| format!("could not call {endpoint}"))
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
) -> Value {
    let mut record = json!({
        "$type": "app.bsky.feed.post",
        "text": text,
        "createdAt": now_timestamp(),
    });

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

async fn response_json(response: Response) -> Result<Value> {
    let status = response.status();
    let text = response
        .text()
        .await
        .context("could not read response body")?;
    if !status.is_success() {
        return Err(anyhow!("Bluesky API returned {status}: {text}"));
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
    use super::*;

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
        );
        assert_eq!(reply["reply"]["root"]["cid"], "rootcid");
        assert_eq!(reply["reply"]["parent"]["cid"], "postcid");

        let quote = post_record_json("quote text", None, Some(subject));
        assert_eq!(quote["embed"]["$type"], "app.bsky.embed.record");
        assert_eq!(quote["embed"]["record"]["cid"], "postcid");
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

    #[test]
    fn parses_unread_notification_count() {
        assert_eq!(unread_notification_count(&json!({"count": 4})).unwrap(), 4);
        assert!(unread_notification_count(&json!({})).is_err());
    }
}
