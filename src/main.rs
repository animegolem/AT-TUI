use std::io::{self, Write};

use anyhow::{Context, Result};
use at_tui::{
    api::BskyClient,
    app::run_tui,
    config::SessionStore,
    media::{MediaCache, RequestedImageProtocol},
};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "at-tui")]
#[command(about = "A Bluesky terminal client prototype")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(long, default_value = "https://bsky.social", global = true)]
    service: String,

    #[arg(long, value_enum, default_value_t = ImageProtocolArg::Auto)]
    image_protocol: ImageProtocolArg,

    #[arg(long)]
    no_images: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    Login {
        #[arg(long)]
        account: Option<String>,

        #[arg(long)]
        handle: Option<String>,

        #[arg(long)]
        app_password: Option<String>,
    },
    Accounts,
    Switch {
        account: String,
    },
    Logout {
        account: Option<String>,
    },
    Session,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ImageProtocolArg {
    Auto,
    Kitty,
    Sixel,
    Iterm2,
    Halfblocks,
}

impl From<ImageProtocolArg> for RequestedImageProtocol {
    fn from(value: ImageProtocolArg) -> Self {
        match value {
            ImageProtocolArg::Auto => RequestedImageProtocol::Auto,
            ImageProtocolArg::Kitty => RequestedImageProtocol::Kitty,
            ImageProtocolArg::Sixel => RequestedImageProtocol::Sixel,
            ImageProtocolArg::Iterm2 => RequestedImageProtocol::Iterm2,
            ImageProtocolArg::Halfblocks => RequestedImageProtocol::Halfblocks,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = SessionStore::new()?;

    match cli.command {
        Some(Command::Login {
            account,
            handle,
            app_password,
        }) => {
            let handle = match handle {
                Some(handle) => handle,
                None => prompt("Handle or email: ")?,
            };
            let app_password = match app_password {
                Some(app_password) => app_password,
                None => rpassword::prompt_password("App password: ")?,
            };
            let session = BskyClient::login_session(&cli.service, &handle, &app_password).await?;
            let label = account.unwrap_or_else(|| session.handle.clone());
            store.save_account(Some(label.clone()), session.clone(), true)?;
            println!("Logged in as @{} ({})", session.handle, session.did);
            println!("Account label: {label}");
            println!("Accounts saved to {}", store.path().display());
        }
        Some(Command::Accounts) => {
            let config = store.load_config()?;
            let active = config.active.as_deref();
            if config.accounts.is_empty() {
                println!("No accounts saved. Run `at-tui login` first.");
            } else {
                for account in config.accounts {
                    let marker = if active.is_some_and(|active| account.matches(active)) {
                        "*"
                    } else {
                        " "
                    };
                    println!(
                        "{marker} {} @{} ({})",
                        account.label, account.session.handle, account.session.did
                    );
                }
            }
        }
        Some(Command::Switch { account }) => {
            let account = store.switch_account(&account)?;
            println!("Switched to {} @{}", account.label, account.session.handle);
        }
        Some(Command::Logout { account }) => match store.remove_account(account.as_deref())? {
            Some(removed) => println!("Removed {} @{}", removed.label, removed.session.handle),
            None => println!("No matching account found"),
        },
        Some(Command::Session) => {
            let account = store.active_account()?;
            let session = account.session;
            let media = MediaCache::disabled();
            println!("Account: {}", account.label);
            println!("Handle: @{}", session.handle);
            println!("DID: {}", session.did);
            println!("Service: {}", session.service);
            println!("Accounts file: {}", store.path().display());
            println!("Images: {}", media.protocol_name());
        }
        None => {
            let account = store
                .active_account()
                .with_context(|| "no saved account; run `at-tui login` first")?;
            let session = account.session;
            let client = BskyClient::new(session, store);
            run_tui(client, cli.image_protocol.into(), cli.no_images).await?;
        }
    }

    Ok(())
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_owned())
}
