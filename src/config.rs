use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
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
    update_lock: Arc<Mutex<()>>,
}

impl SessionStore {
    pub fn new() -> Result<Self> {
        if let Some(config_path) = std::env::var_os("AT_TUI_ACCOUNTS_FILE") {
            return Ok(Self::from_path(PathBuf::from(config_path)));
        }

        let dirs = ProjectDirs::from("dev", "haiti-plan", "at-tui")
            .ok_or_else(|| anyhow!("could not resolve a config directory"))?;
        Ok(Self {
            config_path: dirs.config_dir().join("accounts.json"),
            legacy_session_path: dirs.config_dir().join("session.json"),
            update_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn from_path(config_path: PathBuf) -> Self {
        let legacy_session_path = config_path.with_file_name("session.json");
        Self {
            config_path,
            legacy_session_path,
            update_lock: Arc::new(Mutex::new(())),
        }
    }

    #[cfg(test)]
    pub fn from_paths(config_path: PathBuf, legacy_session_path: PathBuf) -> Self {
        Self {
            config_path,
            legacy_session_path,
            update_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.config_path
    }

    pub fn load(&self) -> Result<Session> {
        self.active_account().map(|account| account.session)
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        let _guard = self.lock_updates()?;
        let mut config = self.load_config_locked()?;
        let label = config
            .accounts
            .iter()
            .find(|account| account.session.did == session.did)
            .map(|account| account.label.clone())
            .unwrap_or_else(|| session.handle.clone());
        let make_active = config.active.is_none();
        upsert_account(&mut config, label, session.clone(), make_active);
        self.save_config_locked(&config)
    }

    pub fn load_config(&self) -> Result<AccountConfig> {
        let _guard = self.lock_updates()?;
        self.load_config_locked()
    }

    fn load_config_locked(&self) -> Result<AccountConfig> {
        if self.config_path.exists() {
            let config = read_config(&self.config_path)?;
            self.remove_legacy_session_locked()?;
            return Ok(config);
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
            self.save_config_locked(&config)?;

            let persisted = read_config(&self.config_path)
                .context("could not verify migrated account configuration")?;
            if persisted != config {
                return Err(anyhow!(
                    "migrated account configuration did not match its durable replacement"
                ));
            }
            self.remove_legacy_session_locked()?;
            return Ok(persisted);
        }

        Ok(AccountConfig::default())
    }

    pub fn save_account(
        &self,
        label: Option<String>,
        session: Session,
        make_active: bool,
    ) -> Result<()> {
        let _guard = self.lock_updates()?;
        let mut config = self.load_config_locked()?;
        let label = label.unwrap_or_else(|| session.handle.clone());
        upsert_account(&mut config, label, session, make_active);
        self.save_config_locked(&config)
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
        let _guard = self.lock_updates()?;
        let mut config = self.load_config_locked()?;
        let account = config
            .accounts
            .iter()
            .find(|account| account.matches(query))
            .cloned()
            .ok_or_else(|| anyhow!("account `{query}` was not found"))?;
        config.active = Some(account.label.clone());
        self.save_config_locked(&config)?;
        Ok(account)
    }

    pub fn remove_account(&self, query: Option<&str>) -> Result<Option<AccountSession>> {
        let _guard = self.lock_updates()?;
        let mut config = self.load_config_locked()?;
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
        self.save_config_locked(&config)?;
        Ok(Some(removed))
    }

    pub fn clear(&self) -> Result<()> {
        let _guard = self.lock_updates()?;
        if self.config_path.exists() {
            fs::remove_file(&self.config_path)
                .with_context(|| format!("could not remove {}", self.config_path.display()))?;
        }
        let backup_path = backup_path(&self.config_path);
        if backup_path.exists() {
            fs::remove_file(&backup_path)
                .with_context(|| format!("could not remove {}", backup_path.display()))?;
        }
        if self.legacy_session_path.exists() {
            fs::remove_file(&self.legacy_session_path).with_context(|| {
                format!("could not remove {}", self.legacy_session_path.display())
            })?;
        }
        if self.config_path.parent().is_some_and(Path::exists) {
            sync_parent_directory(&self.config_path)
                .context("could not make account credential cleanup durable")?;
        }
        Ok(())
    }

    fn lock_updates(&self) -> Result<MutexGuard<'_, ()>> {
        self.update_lock
            .lock()
            .map_err(|_| anyhow!("account configuration update lock was poisoned"))
    }

    fn save_config_locked(&self, config: &AccountConfig) -> Result<()> {
        let mut body = serde_json::to_vec_pretty(config)
            .context("could not serialize account configuration")?;
        body.push(b'\n');

        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }

        if self.config_path.exists() {
            let previous = fs::read(&self.config_path).with_context(|| {
                format!("could not read {} for backup", self.config_path.display())
            })?;
            let backup_path = backup_path(&self.config_path);
            atomic_replace(&backup_path, &previous).with_context(|| {
                format!(
                    "could not preserve previous account configuration at {}",
                    backup_path.display()
                )
            })?;
        }

        atomic_replace(&self.config_path, &body)
            .with_context(|| format!("could not replace {}", self.config_path.display()))
    }

    fn remove_legacy_session_locked(&self) -> Result<()> {
        if !self.legacy_session_path.exists() {
            return Ok(());
        }

        fs::remove_file(&self.legacy_session_path).with_context(|| {
            format!(
                "could not remove migrated legacy session {}",
                self.legacy_session_path.display()
            )
        })?;
        sync_parent_directory(&self.legacy_session_path)
            .context("could not make legacy session cleanup durable")
    }
}

fn read_config(path: &Path) -> Result<AccountConfig> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    serde_json::from_str(&contents).with_context(|| {
        format!(
            "could not parse {}; the last backup, if present, is {}",
            path.display(),
            backup_path(path).display()
        )
    })
}

fn backup_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("accounts.json");
    path.with_file_name(format!("{file_name}.bak"))
}

fn atomic_replace(path: &Path, body: &[u8]) -> Result<()> {
    atomic_replace_with(path, |file| file.write_all(body))
}

fn atomic_replace_with<F>(path: &Path, write_body: F) -> Result<()>
where
    F: FnOnce(&mut File) -> io::Result<()>,
{
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("{} has no valid file name", path.display()))?;
    let temp_path = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(&temp_path)
        .with_context(|| format!("could not create {}", temp_path.display()))?;

    let staged = write_body(&mut file)
        .and_then(|_| file.sync_all())
        .with_context(|| format!("could not stage {}", path.display()));
    drop(file);

    if let Err(error) = staged {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error).with_context(|| {
            format!(
                "could not rename {} over {}",
                temp_path.display(),
                path.display()
            )
        });
    }

    sync_parent_directory(path)
        .with_context(|| format!("could not sync directory for {}", path.display()))
}

fn sync_parent_directory(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent directory", path.display()))?;
    File::open(parent)
        .with_context(|| format!("could not open {} for sync", parent.display()))?
        .sync_all()
        .with_context(|| format!("could not sync {}", parent.display()))
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

    fn session(handle: &str, did: &str, access_jwt: &str, refresh_jwt: &str) -> Session {
        Session {
            service: "https://bsky.social".into(),
            handle: handle.into(),
            did: did.into(),
            access_jwt: access_jwt.into(),
            refresh_jwt: refresh_jwt.into(),
        }
    }

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
        assert!(!legacy_path.exists());
        assert_eq!(store.list_accounts().unwrap().len(), 1);
    }

    #[test]
    fn malformed_config_fails_closed_without_replacing_contents() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("accounts.json");
        let store = SessionStore::from_path(config_path.clone());
        let malformed = b"{ definitely not valid account json\n";
        fs::write(&config_path, malformed).unwrap();

        let error = store
            .save(&session(
                "alice.test",
                "did:plc:alice",
                "new-access",
                "new-refresh",
            ))
            .unwrap_err();

        assert!(format!("{error:#}").contains("could not parse"));
        assert_eq!(fs::read(&config_path).unwrap(), malformed);
        assert!(!backup_path(&config_path).exists());
    }

    #[test]
    fn interrupted_stage_preserves_original_and_cleans_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("accounts.json");
        let original = b"last readable configuration\n";
        fs::write(&config_path, original).unwrap();

        let error = atomic_replace_with(&config_path, |file| {
            file.write_all(b"partial replacement")?;
            Err(io::Error::other("simulated interruption"))
        })
        .unwrap_err();

        assert!(format!("{error:#}").contains("simulated interruption"));
        assert_eq!(fs::read(&config_path).unwrap(), original);
        let entries = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, vec![std::ffi::OsString::from("accounts.json")]);
    }

    #[cfg(unix)]
    #[test]
    fn configuration_and_backup_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("accounts.json");
        let store = SessionStore::from_path(config_path.clone());
        let alice = session(
            "alice.test",
            "did:plc:alice",
            "alice-access",
            "alice-refresh",
        );
        store.save(&alice).unwrap();
        store
            .save(&Session {
                access_jwt: "rotated-access".into(),
                refresh_jwt: "rotated-refresh".into(),
                ..alice
            })
            .unwrap();

        assert_eq!(
            fs::metadata(&config_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(backup_path(&config_path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn backup_is_one_version_deep() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("accounts.json");
        let store = SessionStore::from_path(config_path.clone());
        let original = session(
            "alice.test",
            "did:plc:alice",
            "original-access",
            "original-refresh",
        );
        store.save(&original).unwrap();

        let second = Session {
            access_jwt: "second-access".into(),
            refresh_jwt: "second-refresh".into(),
            ..original
        };
        store.save(&second).unwrap();
        let third = Session {
            access_jwt: "third-access".into(),
            refresh_jwt: "third-refresh".into(),
            ..second.clone()
        };
        store.save(&third).unwrap();

        let backup = read_config(&backup_path(&config_path)).unwrap();
        assert_eq!(backup.accounts.len(), 1);
        assert_eq!(backup.accounts[0].session, second);
        assert_eq!(store.load().unwrap(), third);
        assert_eq!(
            fs::read_dir(dir.path())
                .unwrap()
                .filter_map(|entry| {
                    let name = entry.ok()?.file_name();
                    name.to_string_lossy().contains(".tmp").then_some(name)
                })
                .count(),
            0
        );
    }

    #[test]
    fn clear_removes_primary_backup_and_legacy_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("accounts.json");
        let legacy_path = dir.path().join("session.json");
        let store = SessionStore::from_paths(config_path.clone(), legacy_path.clone());
        let alice = session(
            "alice.test",
            "did:plc:alice",
            "alice-access",
            "alice-refresh",
        );
        store.save(&alice).unwrap();
        store.save(&alice).unwrap();
        fs::write(&legacy_path, b"stale credential").unwrap();

        store.clear().unwrap();

        assert!(!config_path.exists());
        assert!(!backup_path(&config_path).exists());
        assert!(!legacy_path.exists());
    }

    #[test]
    fn completed_migration_does_not_reimport_stale_legacy_session() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("accounts.json");
        let legacy_path = dir.path().join("session.json");
        let store = SessionStore::from_paths(config_path, legacy_path.clone());
        let migrated = session(
            "alice.test",
            "did:plc:alice",
            "alice-access",
            "alice-refresh",
        );
        fs::write(&legacy_path, serde_json::to_vec_pretty(&migrated).unwrap()).unwrap();
        assert_eq!(store.load().unwrap(), migrated);

        let stale = session(
            "stale.test",
            "did:plc:stale",
            "stale-access",
            "stale-refresh",
        );
        fs::write(&legacy_path, serde_json::to_vec_pretty(&stale).unwrap()).unwrap();

        assert_eq!(store.load().unwrap(), migrated);
        assert_eq!(store.list_accounts().unwrap().len(), 1);
        assert!(!legacy_path.exists());
    }

    #[test]
    fn concurrent_refreshes_preserve_both_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().join("accounts.json"));
        let alice = session(
            "alice.test",
            "did:plc:alice",
            "alice-old-access",
            "alice-old-refresh",
        );
        let bob = session(
            "bob.test",
            "did:plc:bob",
            "bob-old-access",
            "bob-old-refresh",
        );
        store
            .save_account(Some("main".into()), alice.clone(), true)
            .unwrap();
        store
            .save_account(Some("alt".into()), bob.clone(), false)
            .unwrap();

        let barrier = Arc::new(std::sync::Barrier::new(3));
        let alice_store = store.clone();
        let alice_barrier = barrier.clone();
        let alice_refresh = std::thread::spawn(move || {
            alice_barrier.wait();
            alice_store.save(&Session {
                access_jwt: "alice-new-access".into(),
                refresh_jwt: "alice-new-refresh".into(),
                ..alice
            })
        });
        let bob_store = store.clone();
        let bob_barrier = barrier.clone();
        let bob_refresh = std::thread::spawn(move || {
            bob_barrier.wait();
            bob_store.save(&Session {
                access_jwt: "bob-new-access".into(),
                refresh_jwt: "bob-new-refresh".into(),
                ..bob
            })
        });
        barrier.wait();
        alice_refresh.join().unwrap().unwrap();
        bob_refresh.join().unwrap().unwrap();

        let config = store.load_config().unwrap();
        assert_eq!(config.active.as_deref(), Some("main"));
        assert_eq!(config.accounts.len(), 2);
        assert_eq!(
            config
                .accounts
                .iter()
                .find(|account| account.label == "main")
                .unwrap()
                .session
                .access_jwt,
            "alice-new-access"
        );
        assert_eq!(
            config
                .accounts
                .iter()
                .find(|account| account.label == "alt")
                .unwrap()
                .session
                .access_jwt,
            "bob-new-access"
        );
    }
}
