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

- `j` / Down: move down
- `k` / Up: move up
- `g`: top
- `G`: bottom
- `l` / Right / Enter: open replies/thread for the selected post
- `h` / Left / Esc: go back
- `[` / `]`: previous/next saved feed
- Space: open media overlay for selected post
- `U`: merge pending new posts and jump to top
- `F`: like/unlike selected post
- `R`: repost/unrepost selected post
- `p`: compose a new text post
- `c`: reply to selected post
- `Q`: quote selected post
- `u`: open selected post's link, or show a picker when multiple links exist
- `o`: open selected post's quoted post
- `/`: search within loaded posts in the current view
- `n`: next search match
- `r`: reload current view
- `?`: menu with keys, account, feed, and image settings
- `q`: quit

Media overlay:

- `h` / Left: previous item
- `l` / Right: next item
- Enter / `p`: decode video frames when selected media is a video
- `u`: open selected video externally
- Space / Esc: close overlay

Composer:

- Type normally; Enter inserts a newline
- Ctrl-S: send
- Esc: cancel

Link picker:

- `j` / Down: next link
- `k` / Up: previous link
- Enter / `u`: open selected link in the default browser
- Esc: close picker

## Timeline Semantics

The primary list uses Bluesky's authenticated home timeline endpoint. Reposts are shown as feed items with a compact `⟳ @handle reposted` context line when the API marks the item with a repost reason. Replies are shown with an inline parent preview when the API includes reply context.

The app reads the `home` feed view preference from `app.bsky.actor.getPreferences` and applies reply, repost, and quote-post hiding locally as a safety net.

Saved feeds are read from `savedFeedsPrefV2` and legacy `savedFeedsPref`. Feed-generator URIs are switchable with `[` and `]`; saved lists are ignored for now. The app also adds a local `Your Posts` feed for the active account.

## Scope

OAuth, image/video upload, notifications, DMs, moderation controls, list feeds, and article previews are not implemented yet. Terminal video playback is experimental and requires `ffmpeg`.
