use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Response, StatusCode};
use serde_json::{Value, json};

use crate::config::{Session, SessionStore};

#[derive(Debug, Clone)]
pub struct BskyClient {
    http: Client,
    store: SessionStore,
    session: Session,
}

impl BskyClient {
    pub fn new(session: Session, store: SessionStore) -> Self {
        Self {
            http: Client::new(),
            store,
            session,
        }
    }

    pub async fn login(
        service: &str,
        identifier: &str,
        app_password: &str,
        store: &SessionStore,
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
        store.save(&session)?;
        Ok(session)
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub async fn get_timeline(&mut self, cursor: Option<&str>, limit: u16) -> Result<Value> {
        let mut query = vec![("limit".to_owned(), limit.to_string())];
        if let Some(cursor) = cursor {
            query.push(("cursor".to_owned(), cursor.to_owned()));
        }
        self.get("app.bsky.feed.getTimeline", &query).await
    }

    pub async fn get_preferences(&mut self) -> Result<Value> {
        let query: Vec<(String, String)> = Vec::new();
        self.get("app.bsky.actor.getPreferences", &query).await
    }

    pub async fn get_post_thread(&mut self, uri: &str) -> Result<Value> {
        let query = post_thread_query(uri);
        self.get("app.bsky.feed.getPostThread", &query).await
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

    async fn send_get(&self, endpoint: &str, query: &[(String, String)]) -> Result<Response> {
        self.http
            .get(xrpc_url(&self.session.service, endpoint))
            .bearer_auth(&self.session.access_jwt)
            .query(query)
            .send()
            .await
            .with_context(|| format!("could not call {endpoint}"))
    }

    async fn refresh_session(&mut self) -> Result<()> {
        let response = self
            .http
            .post(xrpc_url(
                &self.session.service,
                "com.atproto.server.refreshSession",
            ))
            .bearer_auth(&self.session.refresh_jwt)
            .send()
            .await
            .context("could not refresh Bluesky session")?;

        let value = response_json(response).await?;
        self.session.access_jwt = required_string(&value, "accessJwt")?;
        self.session.refresh_jwt = required_string(&value, "refreshJwt")?;
        self.session.handle = value
            .get("handle")
            .and_then(Value::as_str)
            .unwrap_or(&self.session.handle)
            .to_owned();
        self.session.did = value
            .get("did")
            .and_then(Value::as_str)
            .unwrap_or(&self.session.did)
            .to_owned();
        self.store.save(&self.session)?;
        Ok(())
    }
}

fn xrpc_url(service: &str, endpoint: &str) -> String {
    format!("{}/xrpc/{endpoint}", service.trim_end_matches('/'))
}

fn post_thread_query(uri: &str) -> Vec<(String, String)> {
    vec![
        ("uri".to_owned(), uri.to_owned()),
        ("depth".to_owned(), "8".to_owned()),
        ("parentHeight".to_owned(), "80".to_owned()),
    ]
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
}
