use std::{env, path::PathBuf, process::Command};

use anyhow::{Context, Result};

const MPV_ENV: &str = "AT_TUI_MPV";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpvPlayer {
    executable: PathBuf,
}

impl MpvPlayer {
    pub fn discover() -> Option<Self> {
        let executable = env::var_os(MPV_ENV)
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .or_else(find_mpv_on_path)
            .or_else(|| {
                [
                    "/opt/homebrew/opt/mpv/bin/mpv",
                    "/usr/local/opt/mpv/bin/mpv",
                    "/Applications/mpv.app/Contents/MacOS/mpv",
                ]
                .into_iter()
                .map(PathBuf::from)
                .find(|path| path.is_file())
            })?;
        Some(Self { executable })
    }

    pub fn command(&self, playlist_url: &str) -> Command {
        command_for(&self.executable, playlist_url)
    }

    pub fn play(&self, playlist_url: &str) -> Result<std::process::ExitStatus> {
        self.command(playlist_url)
            .status()
            .with_context(|| format!("could not start {}", self.executable.display()))
    }

    pub fn executable(&self) -> &std::path::Path {
        &self.executable
    }
}

fn find_mpv_on_path() -> Option<PathBuf> {
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join("mpv"))
            .find(|candidate| candidate.is_file())
    })
}

fn command_for(executable: &std::path::Path, playlist_url: &str) -> Command {
    let mut command = Command::new(executable);
    command.args([
        "--no-config",
        "--vo=kitty",
        "--vo-kitty-use-shm=yes",
        "--vo-kitty-alt-screen=yes",
        "--keep-open=no",
        "--save-position-on-quit=no",
        "--ytdl=no",
        "--msg-level=all=warn",
        "--",
        playlist_url,
    ]);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_uses_kitty_shared_memory_and_treats_url_as_data() {
        let command = command_for(std::path::Path::new("/test/mpv"), "--playlist-looking-url");
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(command.get_program(), "/test/mpv");
        assert!(args.contains(&"--vo=kitty".to_owned()));
        assert!(args.contains(&"--vo-kitty-use-shm=yes".to_owned()));
        assert_eq!(
            &args[args.len() - 2..],
            &["--".to_owned(), "--playlist-looking-url".to_owned()]
        );
    }
}
