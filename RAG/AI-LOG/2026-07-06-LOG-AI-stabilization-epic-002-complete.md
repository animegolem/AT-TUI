---
node_id: 2026-07-06-LOG-AI-stabilization-epic-002-complete
tags:
  - AI-log
  - development-summary
  - at-tui
  - stabilization
closed_tickets:
  - AI-IMP-002-1
  - AI-IMP-002-2
  - AI-IMP-002-3
  - AI-IMP-002-4
  - AI-IMP-002-5
  - AI-IMP-002-6
  - AI-IMP-002-7
created_date: 2026-07-06
related_files:
  - src/api.rs
  - src/app.rs
  - src/media.rs
  - src/navigation.rs
  - src/ui.rs
  - Cargo.toml
  - README.md
confidence_score: 0.92
---

# 2026-07-06-LOG-AI-stabilization-epic-002-complete

## Work Completed
Full architecture/bug review of the codebase, then cut and completed AI-EPIC-002 (stabilization to daily-driver) in one session: all seven tickets. Root-caused the "stops updating when idle" defect to by-value JWT clones being stranded by ATProto refresh-token rotation; fixed with a shared `Arc<RwLock<Session>>` and single-flight refresh. Landed event-loop hygiene (Press filter, wheel scroll, dirty-flag rendering), pagination for profile/notification views, root-in-place feed loads, the reworked mnemonic keymap with layered `q` and page scrolling, image disk-cache read-through with prefetch/thumb-first/LRU, posting correctness (link+mention facets, grapheme limit, delete-own-post with confirm), and the braille spinner + box-drawing quote frames. Test suite grew 86 → 114, gate green throughout (fmt, tests, clippy -D warnings, build). Smoke-tested the binary against the live saved account (`at-tui session`). A shareable review artifact with the architecture map and ranked findings exists (claude.ai artifact "at-tui review", updated post-session).

## Session Commits
- 6d68e1d Land pre-session follow/unfollow, notifications, and profile work (baseline of uncommitted prior work so tickets commit cleanly).
- 5d3642a Cut AI-EPIC-002 stabilization epic and tickets 002-1..7.
- 4220cf2 AI-IMP-002-1: share one live session across client clones.
- a314237 AI-IMP-002-2: press-filtered input, wheel scrolling, dirty-flag redraw.
- db30f27 AI-IMP-002-3: paginate profile/notification views, preserve nav stack.
- 2b48af3 AI-IMP-002-4: mnemonic keymap with layered q and page scrolling.
- 2b343c2 AI-IMP-002-5: disk read-through, prefetch, thumb-first, LRU media cache.
- 853e82f AI-IMP-002-6: facets on outgoing posts, grapheme limit, delete own post.
- 6645f98 AI-IMP-002-7: braille statusline spinner and box-drawing quote frames.
- (this commit) Close AI-EPIC-002 and add session log.

## Issues Encountered
- `BskyClient::session()` kept its name but now returns an owned snapshot; every call site compiled unchanged, avoiding a wide refactor.
- Single-flight refresh is unit-tested via gate-hold/rotate/release; a true concurrent-401 integration test would need an HTTP mock (deferred).
- Grapheme test first used 150 two-char emoji = exactly 300 chars; bumped to 151.
- Known cosmetic nit (002-7): media-summary rows inside quote frames render their `│` in summary color, not frame yellow.
- Known theoretical edge (002-5): a displayed image could be LRU-evicted if 48+ loads complete without a redraw touching it; render-access touch makes this effectively unreachable.
- Mention facets use a conservative ASCII handle scanner; unicode/punycode handles post as plain text.

## Tests Added
28 net-new tests: session sharing across clones and refresh single-flight skip (api); badge retention/offline threshold, release-key filtering, wheel selection, drain count, profile/notification pagination, stack preservation on feed load, keymap registry incl. retired keys, q layering, page motion, delete guard/confirm/removal, grapheme counting (app); replace_root, move_by clamping (navigation); disk-cache read-through, corrupt-entry discard, LRU bound with touch, render-key thumb-first preference, prefetch URL coverage (media); box-drawing quote frame (ui).

## Next Steps
- Feature epic candidates (from the review's missing-surfaces list): mute/block, global search (`searchPosts`/`searchActors`), follower/following list views, image upload + alt text, list feeds; larger: OAuth, DMs.
- Visual pass candidates: deterministic author colors from DID hash, gutter-bar selection, inline facet styling of links/mentions/tags, inline timeline thumbnails.
- Verify the 24h-idle success metric empirically by leaving the app running past two token expiries and confirming the timeline keeps refreshing with no offline segment.
- Consider an HTTP-mock integration test for concurrent 401 refresh.
