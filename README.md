# at-tui

A Bluesky terminal client prototype.

## What Works

- App-password login against Bluesky's XRPC API.
- Multi-account app-password sessions in the platform config directory, with startup refresh.
- Authenticated Bluesky home timeline (`app.bsky.feed.getTimeline`) with cursor pagination.
- Saved feed switching through `app.bsky.feed.getFeed`.
- Your Posts feed through `app.bsky.feed.getAuthorFeed`.
- Background refresh with pending-new-post counts.
- Home feed preference reads for replies, reposts, and quote posts.
- Repost and reply context rendering.
- Stack-based navigation inspired by Ranger/Yazi.
- Single-column timeline/thread/feed layout at every terminal width.
- Background loading for pagination, threads, feeds, account switches, images, and link opening.
- Vim-style movement with `j`/`k`.
- Thread/reply navigation with `l`, right arrow, or Enter.
- Back navigation with `h`, left arrow, or Esc.
- Inline quote-post rendering, with `o` to open the quoted post as its own stack level.
- Spacebar media preview overlay for post and quote-post images/videos.
- Experimental terminal video frame decoding through `ffmpeg`.
- Link extraction from external cards, rich-text facets, and plain URLs, with `u` to open in the default browser.
- Like/unlike, repost/unrepost, text posts, replies, and quote posts.
- Filled/outline heart state for liked posts and highlighted repost state.
- Unread notification count polling in the statusline.
- Image fetching and terminal image rendering through `ratatui-image`.
- Image protocol selection with `--image-protocol auto|kitty|sixel|iterm2|halfblocks`.
- `--no-images` fallback mode.
- Unicode text-symbol counters: `↩`, `⟳`, `♥`, and `❞`.

## Usage

```sh
cargo run -- login
cargo run --
```

Manage accounts:

```sh
cargo run -- login --account main
cargo run -- accounts
cargo run -- switch main
cargo run -- logout main
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

## Timeline Semantics

The primary list uses Bluesky's authenticated home timeline endpoint. Reposts are shown as feed items with a compact `⟳ @handle reposted` context line when the API marks the item with a repost reason. Replies are shown with an inline parent preview when the API includes reply context.

The app reads the `home` feed view preference from `app.bsky.actor.getPreferences` and applies reply, repost, and quote-post hiding locally as a safety net.

Saved feeds are read from `savedFeedsPrefV2` and legacy `savedFeedsPref`. Feed-generator URIs are switchable with `[` and `]`; saved lists are ignored for now. The app also adds a local `Your Posts` feed for the active account.

## Scope

OAuth, image/video upload, notifications, DMs, moderation controls, list feeds, and article previews are not implemented yet. Terminal video playback is experimental and requires `ffmpeg`.
