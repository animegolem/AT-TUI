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
#[command(about = "A read-only Bluesky terminal client prototype")]
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
        handle: Option<String>,

        #[arg(long)]
        app_password: Option<String>,
    },
    Logout,
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
            let session = BskyClient::login(&cli.service, &handle, &app_password, &store).await?;
            println!("Logged in as @{} ({})", session.handle, session.did);
            println!("Session saved to {}", store.path().display());
        }
        Some(Command::Logout) => {
            store.clear()?;
            println!("Removed {}", store.path().display());
        }
        Some(Command::Session) => {
            let session = store.load()?;
            let media = MediaCache::disabled();
            println!("Handle: @{}", session.handle);
            println!("DID: {}", session.did);
            println!("Service: {}", session.service);
            println!("Session file: {}", store.path().display());
            println!("Images: {}", media.protocol_name());
        }
        None => {
            let session = store
                .load()
                .with_context(|| "no saved session; run `at-tui login` first")?;
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
