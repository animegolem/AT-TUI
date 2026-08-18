use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Output},
    thread::{self, JoinHandle},
    time::Duration,
};

use at_tui::config::{Session, SessionStore};

fn spawn_login_server() -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        assert!(
            String::from_utf8_lossy(&request)
                .starts_with("POST /xrpc/com.atproto.server.createSession")
        );

        let body = r#"{"handle":"alice.test","did":"did:plc:alice","accessJwt":"alice-access","refreshJwt":"alice-refresh"}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
        stream.flush().unwrap();
    });
    (format!("http://{address}"), handle)
}

fn run(config_path: &std::path::Path, args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_at-tui"))
        .env("AT_TUI_ACCOUNTS_FILE", config_path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "command {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn account_cli_round_trip_uses_isolated_store() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("accounts.json");
    let (service, server) = spawn_login_server();

    let login = run(
        &config_path,
        &[
            "--service",
            &service,
            "login",
            "--account",
            "main",
            "--handle",
            "alice.test",
            "--app-password",
            "test-password",
        ],
    );
    server.join().unwrap();
    assert!(String::from_utf8_lossy(&login.stdout).contains("Logged in as @alice.test"));

    let store = SessionStore::from_path(config_path.clone());
    store
        .save_account(
            Some("alt".into()),
            Session {
                service: "https://bsky.social".into(),
                handle: "bob.test".into(),
                did: "did:plc:bob".into(),
                access_jwt: "bob-access".into(),
                refresh_jwt: "bob-refresh".into(),
            },
            false,
        )
        .unwrap();

    let accounts = run(&config_path, &["accounts"]);
    let accounts = String::from_utf8_lossy(&accounts.stdout);
    assert!(accounts.contains("* main @alice.test"));
    assert!(accounts.contains("  alt @bob.test"));

    let switched = run(&config_path, &["switch", "alt"]);
    assert!(String::from_utf8_lossy(&switched.stdout).contains("Switched to alt @bob.test"));

    let active = run(&config_path, &["session"]);
    assert!(String::from_utf8_lossy(&active.stdout).contains("Account: alt"));

    let logout = run(&config_path, &["logout", "alt"]);
    assert!(String::from_utf8_lossy(&logout.stdout).contains("Removed alt @bob.test"));

    let final_accounts = run(&config_path, &["accounts"]);
    let final_accounts = String::from_utf8_lossy(&final_accounts.stdout);
    assert!(final_accounts.contains("* main @alice.test"));
    assert!(!final_accounts.contains("bob.test"));
}
