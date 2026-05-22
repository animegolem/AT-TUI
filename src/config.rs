use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use anyhow::{Context, Result, anyhow};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub service: String,
    pub handle: String,
    pub did: String,
    pub access_jwt: String,
    pub refresh_jwt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountSession {
    pub label: String,
    pub session: Session,
}

impl AccountSession {
    pub fn matches(&self, query: &str) -> bool {
        self.label == query || self.session.handle == query || self.session.did == query
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountConfig {
    pub active: Option<String>,
    pub accounts: Vec<AccountSession>,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    config_path: PathBuf,
    legacy_session_path: PathBuf,
}

impl SessionStore {
    pub fn new() -> Result<Self> {
        let dirs = ProjectDirs::from("dev", "haiti-plan", "at-tui")
            .ok_or_else(|| anyhow!("could not resolve a config directory"))?;
        Ok(Self {
            config_path: dirs.config_dir().join("accounts.json"),
            legacy_session_path: dirs.config_dir().join("session.json"),
        })
    }

    #[cfg(test)]
    pub fn from_path(config_path: PathBuf) -> Self {
        let legacy_session_path = config_path.with_file_name("session.json");
        Self {
            config_path,
            legacy_session_path,
        }
    }

    #[cfg(test)]
    pub fn from_paths(config_path: PathBuf, legacy_session_path: PathBuf) -> Self {
        Self {
            config_path,
            legacy_session_path,
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.config_path
    }

    pub fn load(&self) -> Result<Session> {
        self.active_account().map(|account| account.session)
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        let mut config = self.load_config().unwrap_or_default();
        let label = config
            .accounts
            .iter()
            .find(|account| account.session.did == session.did)
            .map(|account| account.label.clone())
            .unwrap_or_else(|| session.handle.clone());
        let make_active = config.active.is_none();
        upsert_account(&mut config, label, session.clone(), make_active);
        self.save_config(&config)
    }

    pub fn load_config(&self) -> Result<AccountConfig> {
        if self.config_path.exists() {
            let contents = fs::read_to_string(&self.config_path)
                .with_context(|| format!("could not read {}", self.config_path.display()))?;
            return serde_json::from_str(&contents)
                .with_context(|| format!("could not parse {}", self.config_path.display()));
        }

        if self.legacy_session_path.exists() {
            let contents = fs::read_to_string(&self.legacy_session_path).with_context(|| {
                format!("could not read {}", self.legacy_session_path.display())
            })?;
            let session: Session = serde_json::from_str(&contents).with_context(|| {
                format!("could not parse {}", self.legacy_session_path.display())
            })?;
            let config = AccountConfig {
                active: Some(session.handle.clone()),
                accounts: vec![AccountSession {
                    label: session.handle.clone(),
                    session,
                }],
            };
            self.save_config(&config)?;
            return Ok(config);
        }

        Ok(AccountConfig::default())
    }

    pub fn save_account(
        &self,
        label: Option<String>,
        session: Session,
        make_active: bool,
    ) -> Result<()> {
        let mut config = self.load_config().unwrap_or_default();
        let label = label.unwrap_or_else(|| session.handle.clone());
        upsert_account(&mut config, label, session, make_active);
        self.save_config(&config)
    }

    pub fn active_account(&self) -> Result<AccountSession> {
        let config = self.load_config()?;
        let active = config
            .active
            .as_deref()
            .ok_or_else(|| anyhow!("no active account; run `at-tui login` first"))?;
        config
            .accounts
            .into_iter()
            .find(|account| account.matches(active))
            .ok_or_else(|| anyhow!("active account `{active}` was not found"))
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountSession>> {
        Ok(self.load_config()?.accounts)
    }

    pub fn switch_account(&self, query: &str) -> Result<AccountSession> {
        let mut config = self.load_config()?;
        let account = config
            .accounts
            .iter()
            .find(|account| account.matches(query))
            .cloned()
            .ok_or_else(|| anyhow!("account `{query}` was not found"))?;
        config.active = Some(account.label.clone());
        self.save_config(&config)?;
        Ok(account)
    }

    pub fn remove_account(&self, query: Option<&str>) -> Result<Option<AccountSession>> {
        let mut config = self.load_config()?;
        let target = match query {
            Some(query) => query.to_owned(),
            None => config
                .active
                .clone()
                .ok_or_else(|| anyhow!("no active account to remove"))?,
        };
        let Some(index) = config
            .accounts
            .iter()
            .position(|account| account.matches(&target))
        else {
            return Ok(None);
        };

        let removed = config.accounts.remove(index);
        if config
            .active
            .as_ref()
            .is_some_and(|active| removed.matches(active))
        {
            config.active = config.accounts.first().map(|account| account.label.clone());
        }
        self.save_config(&config)?;
        Ok(Some(removed))
    }

    pub fn clear(&self) -> Result<()> {
        if self.config_path.exists() {
            fs::remove_file(&self.config_path)
                .with_context(|| format!("could not remove {}", self.config_path.display()))?;
        }
        if self.legacy_session_path.exists() {
            fs::remove_file(&self.legacy_session_path).with_context(|| {
                format!("could not remove {}", self.legacy_session_path.display())
            })?;
        }
        Ok(())
    }

    fn save_config(&self, config: &AccountConfig) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }

        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        let mut file = options
            .open(&self.config_path)
            .with_context(|| format!("could not open {}", self.config_path.display()))?;
        let body = serde_json::to_vec_pretty(config)?;
        file.write_all(&body)
            .with_context(|| format!("could not write {}", self.config_path.display()))?;
        file.write_all(b"\n")?;
        Ok(())
    }
}

fn upsert_account(config: &mut AccountConfig, label: String, session: Session, make_active: bool) {
    if let Some(existing) = config
        .accounts
        .iter_mut()
        .find(|account| account.label == label || account.session.did == session.did)
    {
        existing.label = label.clone();
        existing.session = session;
    } else {
        config.accounts.push(AccountSession {
            label: label.clone(),
            session,
        });
    }

    if make_active || config.active.is_none() {
        config.active = Some(label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().join("accounts.json"));
        let session = Session {
            service: "https://bsky.social".into(),
            handle: "alice.test".into(),
            did: "did:plc:alice".into(),
            access_jwt: "access".into(),
            refresh_jwt: "refresh".into(),
        };

        store.save(&session).unwrap();
        assert_eq!(store.load().unwrap(), session);
    }

    #[test]
    fn saves_switches_and_removes_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().join("accounts.json"));
        let alice = Session {
            service: "https://bsky.social".into(),
            handle: "alice.test".into(),
            did: "did:plc:alice".into(),
            access_jwt: "alice-access".into(),
            refresh_jwt: "alice-refresh".into(),
        };
        let bob = Session {
            service: "https://bsky.social".into(),
            handle: "bob.test".into(),
            did: "did:plc:bob".into(),
            access_jwt: "bob-access".into(),
            refresh_jwt: "bob-refresh".into(),
        };

        store
            .save_account(Some("main".into()), alice.clone(), true)
            .unwrap();
        store
            .save_account(Some("alt".into()), bob.clone(), true)
            .unwrap();

        assert_eq!(store.load().unwrap(), bob);
        assert_eq!(store.switch_account("main").unwrap().session, alice);
        assert_eq!(store.list_accounts().unwrap().len(), 2);
        assert_eq!(
            store.remove_account(Some("main")).unwrap().unwrap().label,
            "main"
        );
        assert_eq!(store.list_accounts().unwrap().len(), 1);
    }

    #[test]
    fn refreshed_session_updates_same_account() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().join("accounts.json"));
        let session = Session {
            service: "https://bsky.social".into(),
            handle: "alice.test".into(),
            did: "did:plc:alice".into(),
            access_jwt: "old-access".into(),
            refresh_jwt: "old-refresh".into(),
        };
        store
            .save_account(Some("main".into()), session.clone(), true)
            .unwrap();

        let refreshed = Session {
            access_jwt: "new-access".into(),
            refresh_jwt: "new-refresh".into(),
            ..session
        };
        store.save(&refreshed).unwrap();

        let config = store.load_config().unwrap();
        assert_eq!(config.active.as_deref(), Some("main"));
        assert_eq!(config.accounts.len(), 1);
        assert_eq!(config.accounts[0].label, "main");
        assert_eq!(config.accounts[0].session.access_jwt, "new-access");
        assert_eq!(config.accounts[0].session.refresh_jwt, "new-refresh");
    }

    #[test]
    fn migrates_legacy_single_session() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("accounts.json");
        let legacy_path = dir.path().join("session.json");
        let store = SessionStore::from_paths(config_path.clone(), legacy_path.clone());
        let session = Session {
            service: "https://bsky.social".into(),
            handle: "alice.test".into(),
            did: "did:plc:alice".into(),
            access_jwt: "access".into(),
            refresh_jwt: "refresh".into(),
        };
        fs::write(
            &legacy_path,
            serde_json::to_string_pretty(&session).unwrap(),
        )
        .unwrap();

        assert_eq!(store.load().unwrap(), session);
        assert!(config_path.exists());
        assert_eq!(store.list_accounts().unwrap().len(), 1);
    }
}
