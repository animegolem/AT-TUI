# at-tui

A read-only Bluesky terminal client prototype.

## What Works

- App-password login against Bluesky's XRPC API.
- Saved local session in the platform config directory.
- Authenticated Bluesky home timeline (`app.bsky.feed.getTimeline`) with cursor pagination.
- Home feed preference reads for replies, reposts, and quote posts.
- Repost and reply context rendering.
- Stack-based navigation inspired by Ranger/Yazi.
- Vim-style movement with `j`/`k`.
- Thread/reply navigation with `l`, right arrow, or Enter.
- Back navigation with `h`, left arrow, or Esc.
- Inline quote-post rendering, with `o` to open the quoted post as its own stack level.
- Image fetching and terminal image rendering through `ratatui-image`.
- Image protocol selection with `--image-protocol auto|kitty|sixel|iterm2|halfblocks`.
- `--no-images` fallback mode.
- Unicode text-symbol counters: `↩`, `⟳`, `♥`, and `❞`.

## Usage

```sh
cargo run -- login
cargo run --
```

For Ghostty, auto-detection should normally select Kitty graphics. You can force it:

```sh
cargo run -- --image-protocol kitty
```

Disable image rendering entirely:

```sh
cargo run -- --no-images
```

## Keys

- `j` / Down: move down
- `k` / Up: move up
- `g`: top
- `G`: bottom
- `l` / Right / Enter: open replies/thread for the selected post
- `h` / Left / Esc: go back
- `o`: open selected post's quoted post
- `/`: search within loaded posts in the current view
- `n`: next search match
- `r`: reload current view
- `q`: quit

## Timeline Semantics

The primary list uses Bluesky's authenticated home timeline endpoint. Reposts are shown as feed items with a compact `⟳ @handle reposted` context line when the API marks the item with a repost reason. Replies are shown with an inline parent preview when the API includes reply context.

The app reads the `home` feed view preference from `app.bsky.actor.getPreferences` and applies reply, repost, and quote-post hiding locally as a safety net.

## Scope

This is intentionally read-only. Posting, liking, reposting, notifications, DMs, moderation controls, custom feeds, OAuth, video playback, and multi-account support are not implemented yet.
