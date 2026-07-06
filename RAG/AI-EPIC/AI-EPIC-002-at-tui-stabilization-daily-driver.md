---
node_id: AI-EPIC-002
tags:
  - EPIC
  - AI
  - at-tui
  - stabilization
  - keymap
  - media
date_created: 2026-07-06
date_completed:
kanban_status: in-progress
AI_IMP_spawned:
  - AI-IMP-002-1
  - AI-IMP-002-2
  - AI-IMP-002-3
  - AI-IMP-002-4
  - AI-IMP-002-5
  - AI-IMP-002-6
  - AI-IMP-002-7
---

# AI-EPIC-002-at-tui-stabilization-daily-driver

## Problem Statement/Feature Scope
AT-TUI cannot currently be left running: after roughly two hours idle the access token expires, the refreshing clone strands the main client with a revoked refresh token, and the app silently stops updating until restart. Around that core defect sit smaller reliability gaps — errors that vanish after two seconds, profile and notification views that dead-end at 50 items, double keystrokes on kitty-protocol terminals, a constant 20 fps idle redraw — plus a keymap that was assigned ad hoc and an image pipeline that re-downloads everything on every run. This epic takes the app from "works in a demo" to "trustworthy daily driver."

## Proposed Solution(s)
Six phases, ordered so each layer stands on a stable one below it.

**Phase 1 — session stability (the idle bug).** Share one live `Session` across all `BskyClient` clones via `Arc<tokio::sync::RwLock<Session>>`, single-flight the refresh so concurrent 401s cannot race the token rotation, and make failure visible: a persistent "disconnected — retrying" statusline segment while consecutive polls fail, and no zeroing of the notification badge on error.

**Phase 2 — event-loop hygiene.** Filter key events to `KeyEventKind::Press`; handle mouse wheel as move up/down (justifying the already-enabled mouse capture); redraw only when state changed (dirty flag) instead of unconditionally at 20 fps.

**Phase 3 — view correctness.** Extend `maybe_load_more` beyond `ViewKind::Timeline` so profile and notification views use the cursors they already store; make a completing feed load replace the root view in place instead of discarding the whole navigation stack.

**Phase 4 — keymap rework.** Keep Yazi-style navigation sacred; reassign action keys onto consistent mnemonics (proposed map below), add half-page/page scrolling, update help overlay and README together.

**Phase 5 — image speed.** Read the disk cache before HTTP (it is currently write-only); prefetch images for the selected ±2 posts as the cursor moves; render thumbnails first and swap in fullsize; cap the in-memory decoded cache with LRU eviction.

**Phase 6 — posting correctness.** Generate link/mention facets for outgoing posts so they are rich text in other clients; count graphemes rather than chars against the 300 limit; add delete-own-post with confirmation.

### Proposed keymap (decision needed before Phase 4 is cut)
Unchanged and sacred: `h/j/k/l`, arrows, `Enter` open, `Esc` back, `g/G`, `[`/`]` feeds, `Space` media, `/` search, `n` next match, `?` menu, `Ctrl-C` quit.

| Key | Today | Proposed | Rationale |
| --- | --- | --- | --- |
| `q` | quit app | back/close; quits only at timeline root | removes quit-by-accident; matches k9s-style layering |
| `f` | — | like | mnemonic "fav"; lowercase for the most frequent action |
| `F` | like | follow author | small `f` acts on the post, big `F` on the person |
| `b` | — | repost | "boost" — tut/Mastodon TUI convention |
| `r` | reload | reply | mail-client convention; reply is frequent |
| `R` | repost | reload view | shifted = heavier action on the view |
| `Q` | quote (adjacent to quit!) | quote | safe once `q` no longer quits |
| `o` | open quote embed | open link in browser | "open" convention; single link opens, multiple picks |
| `e` | — | view embedded/quoted post | "embed" |
| `u` | open links | load pending new posts | "update"; was `U` |
| `w` | follow | — (freed) | folded into `F` |
| `d` | — | delete own post (Phase 6) | with y/n confirm |
| `Ctrl-d`/`Ctrl-u`, `PgDn`/`PgUp` | — | half-page / page scroll | long-feed necessity |

## Path(s) Not Taken
Global search, follower/following list views, mute/block, image upload, list feeds, OAuth, DMs, inline timeline thumbnails, and theming are deliberately excluded; they belong to a subsequent feature epic once this epic makes the foundation trustworthy. A keymap config file is also excluded — the map is fixed in code this pass.

## Success Metrics
- App left running 24 hours continues refreshing the timeline and notification badge with no restart and no re-login.
- Scrolling past item 50 in profile and notification views loads further pages.
- Exactly one action fires per keypress on kitty-protocol terminals; wheel scrolling moves the selection.
- Idle CPU usage near zero when no video is playing (no redraw without a state change).
- Second launch renders previously seen images without network access.
- A post containing a URL renders as a tappable link in the official Bluesky client.
- Existing gate stays green: fmt, unit tests, clippy -D warnings, build.

## Requirements

### Functional Requirements
- [ ] FR-1: All `BskyClient` clones shall share one live session; a refresh in any task is visible to all.
- [ ] FR-2: Session refresh shall be single-flight; concurrent 401s produce exactly one refresh call.
- [ ] FR-3: Consecutive background-poll failures shall surface a persistent statusline indicator; the unread badge shall retain its last known value on error.
- [ ] FR-4: Key handling shall ignore non-Press key events.
- [ ] FR-5: Mouse wheel shall move the selection up/down.
- [ ] FR-6: The terminal shall redraw only when state has changed.
- [ ] FR-7: Profile and notification views shall paginate using their stored cursors.
- [ ] FR-8: A completing feed load shall replace the root view in place, preserving any pushed views.
- [ ] FR-9: The agreed keymap shall be implemented with help overlay and README updated in the same change.
- [ ] FR-10: Half-page and page scrolling shall be available in all list views.
- [ ] FR-11: Image loads shall check the disk cache before the network.
- [ ] FR-12: Images for posts near the selection shall prefetch in the background.
- [ ] FR-13: Media previews shall render the thumbnail immediately and upgrade to fullsize.
- [ ] FR-14: The in-memory decoded-image cache shall be bounded with LRU eviction.
- [ ] FR-15: Outgoing posts shall include link and mention facets.
- [ ] FR-16: Post length shall be validated by grapheme count.
- [ ] FR-17: Users shall be able to delete their own posts after confirmation.
- [ ] FR-18: The statusline pending indicator shall animate (braille spinner) while tasks run.
- [ ] FR-19: Quote posts shall render inside box-drawing frames consistent with the app chrome.

### Non-Functional Requirements
- No new heavyweight dependencies; `unicode-segmentation` for FR-16 is acceptable.
- Preserve the single-owner state model: tasks report via events, never mutate shared app state (the shared session is the sole, lock-guarded exception).
- Remain keyboard-first and terminal-portable; no Nerd Font or OAuth requirements.
- Every behavioral change lands with unit tests in the existing test style.

## Implementation Breakdown
Cut 2026-07-06 with owner go (visual quick wins added to scope at cut time):
- [ ] [[AI-IMP-002-1-session-sharing-failure-visibility]]: shared session, single-flight refresh, offline indicator (FR-1..3).
- [ ] [[AI-IMP-002-2-event-loop-hygiene]]: Press filter, wheel scroll, dirty-flag redraw (FR-4..6).
- [ ] [[AI-IMP-002-3-view-pagination-stack-preservation]]: profile/notification pagination, root-in-place feed loads (FR-7..8).
- [ ] [[AI-IMP-002-4-keymap-rework-page-scroll]]: agreed keymap plus page motion (FR-9..10).
- [ ] [[AI-IMP-002-5-image-cache-prefetch]]: disk read-through, prefetch, thumb-first, LRU (FR-11..14).
- [ ] [[AI-IMP-002-6-posting-correctness]]: facets, graphemes, delete own post (FR-15..17).
- [ ] [[AI-IMP-002-7-visual-quick-wins]]: braille spinner, box-drawing quote frames (FR-18..19).

Dependency order: 002-1 first; 002-2 → 002-3 → 002-4 serialized (all touch app.rs); 002-5/002-6/002-7 follow, independent of each other.
