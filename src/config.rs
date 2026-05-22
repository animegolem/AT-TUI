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

#[derive(Debug, Clone)]
pub struct SessionStore {
    session_path: PathBuf,
}

impl SessionStore {
    pub fn new() -> Result<Self> {
        let dirs = ProjectDirs::from("dev", "haiti-plan", "at-tui")
            .ok_or_else(|| anyhow!("could not resolve a config directory"))?;
        Ok(Self {
            session_path: dirs.config_dir().join("session.json"),
        })
    }

    #[cfg(test)]
    pub fn from_path(session_path: PathBuf) -> Self {
        Self { session_path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.session_path
    }

    pub fn load(&self) -> Result<Session> {
        let contents = fs::read_to_string(&self.session_path)
            .with_context(|| format!("could not read {}", self.session_path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("could not parse {}", self.session_path.display()))
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        if let Some(parent) = self.session_path.parent() {
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
            .open(&self.session_path)
            .with_context(|| format!("could not open {}", self.session_path.display()))?;
        let body = serde_json::to_vec_pretty(session)?;
        file.write_all(&body)
            .with_context(|| format!("could not write {}", self.session_path.display()))?;
        file.write_all(b"\n")?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        if self.session_path.exists() {
            fs::remove_file(&self.session_path)
                .with_context(|| format!("could not remove {}", self.session_path.display()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().join("session.json"));
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
}
