# at-tui

AT-TUI is a keyboard-first Bluesky client for the terminal. It provides a
single-column interface for reading feeds, browsing threads and profiles, and
interacting with posts.

## Features

- Read and paginate the home timeline, saved feeds, author feeds, profiles,
  threads, and notifications.
- Switch between saved app-password accounts.
- Refresh feeds in the background and show pending posts and unread
  notifications.
- Search the current view and open post links in your default browser.
- Like, repost, follow, create posts, reply, quote posts, and delete your own
  posts.
- Preview images with Kitty, Sixel, iTerm2, or half-block rendering.
- Play Bluesky HLS video and audio through mpv's Kitty output.
- Use the same single-column layout at narrow and wide terminal sizes.

## Requirements

- A Rust toolchain that supports Rust 2024 edition.
- A Bluesky account and app password.
- A terminal supported by `ratatui-image` for image previews. Ghostty normally
  uses the Kitty graphics protocol.
- mpv for optional video playback. Video output requires a terminal that
  supports Kitty graphics.

## Install

Build and install the optimized binary with Cargo:

```sh
cargo install --path . --locked
```

To build without installing, run:

```sh
cargo build --release
./target/release/at-tui --help
```

## Quickstart

Log in with an app password. The command prompts for your handle and password:

```sh
at-tui login
at-tui
```

AT-TUI stores account sessions in your platform configuration directory. To
manage saved accounts, run:

```sh
at-tui login --account main
at-tui accounts
at-tui switch main
at-tui logout main
```

## Configure media

AT-TUI detects the image protocol. To select one explicitly, run:

```sh
at-tui --image-protocol kitty
```

Valid values are `auto`, `kitty`, `sixel`, `iterm2`, and `halfblocks`. To
disable image rendering, run `at-tui --no-images`.

Video playback uses mpv. Open a post's media with Space, select a video, and
press Enter or `p`. mpv controls playback until you press `q` to return to
AT-TUI. Press `u` to open the playlist externally.

AT-TUI searches `PATH`, standard Homebrew locations, and the macOS app bundle
for mpv. Set `AT_TUI_MPV` to an explicit executable path to override discovery.

## Keys

Mouse-wheel selection is handled separately from keyboard bindings.

<!-- BEGIN GENERATED KEYMAP -->
### Normal

- `Ctrl-C` — quit
- `j/k or arrows` — move
- `Ctrl-d/Ctrl-u` — half page
- `PgUp/PgDn` — page
- `g/G` — top/bottom
- `l/Enter/Right` — open selected
- `h/Left` — back
- `Esc` — back/settings
- `q` — back; quit at root
- `P` — profile
- `N` — notifications
- `Space` — media
- `o` — links
- `e` — quoted post
- `f` — like
- `b` — repost
- `F` — follow
- `d` — delete
- `p` — post
- `r` — reply
- `Q` — quote
- `[/]` — previous/next feed
- `/` — search
- `n` — next match
- `R` — reload
- `u` — load pending
- `?` — menu

### Menu

- `Ctrl-C` — quit
- `Esc/?/Enter/q` — close
- `j/k or arrows` — change section
- `Tab/Shift-Tab` — section action
- `[/]` — previous/next feed

### Media overlay

- `Ctrl-C` — quit
- `Space/Esc/q` — close
- `h/l or arrows` — switch
- `Enter/p` — play video
- `u` — open externally

### Composer

- `Ctrl-C` — quit
- `Esc` — cancel
- `Ctrl-S` — send
- `Enter` — newline
- `Backspace` — delete character
- `Text` — type normally

### Link picker

- `Ctrl-C` — quit
- `Esc/q` — close
- `j/k or arrows` — move
- `Enter/u` — open

### Delete confirmation

- `Ctrl-C` — quit
- `y/Y` — delete
- `Any other key` — cancel

### Search

- `Ctrl-C` — quit
- `Esc` — cancel
- `Enter` — search
- `Backspace` — delete character
- `Text` — type query
<!-- END GENERATED KEYMAP -->

## Timeline behavior

The primary list uses Bluesky's authenticated home timeline endpoint. Reposts
include a compact `⟳ @handle reposted` context line. Replies include a parent
preview when the API provides reply context.

AT-TUI reads the `home` feed preference from
`app.bsky.actor.getPreferences`. It also applies reply, repost, and quote-post
filters locally.

AT-TUI reads saved feeds from `savedFeedsPrefV2` and the legacy
`savedFeedsPref`. Use `[` and `]` to switch feed generators. Saved lists are
not available. AT-TUI also adds a local **Your Posts** feed for the active
account.

## Limitations

AT-TUI does not support OAuth, media upload, direct messages, moderation
controls, list feeds, article previews, or server-wide search. Video playback
is experimental and requires mpv and Kitty graphics.

## Develop

Run the project checks before you submit a change:

```sh
cargo fmt -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release
git diff --check
```
