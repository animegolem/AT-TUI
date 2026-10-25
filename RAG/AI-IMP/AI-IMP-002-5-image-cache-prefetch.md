---
node_id: AI-IMP-002-5
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - media
  - performance
kanban_status: completed
depends_on:
parent_epic: [[AI-EPIC-002-at-tui-stabilization-daily-driver]]
confidence_score: 0.85
date_created: 2026-07-06
date_completed: 2026-07-06
---

# AI-IMP-002-5-image-cache-prefetch

## Summary of Issue #1
The image pipeline re-downloads everything, every run: `ImageLoadJob` writes bytes to the sha256-keyed disk cache but never reads it back. Loads also start only when the media overlay opens, so every Space press stares at "Image loading", and the decoded-protocol map grows without bound. Done state: disk hits skip the network, images near the selection prefetch in the background, previews show the thumbnail immediately and upgrade to fullsize, and the in-memory cache is LRU-bounded.

### Out of Scope
Inline timeline thumbnails, video frame caching changes beyond a count cap, cache expiry/GC of the disk directory, avatar rendering.

### Design/Approach
`ImageLoadJob::run`: try `fs::read` on the cache path first; on hit decode without network, on miss download then write (decode moved inside `spawn_blocking` — it is CPU-bound). Prefetch: after each handled input in `App::handle_key`, queue thumbnail loads for posts within ±2 of the selection via the existing `queue_image_loads` (states dedupe repeats). Thumb-first: `PreviewImage` carries both `thumb_url` and fullsize `url`; `render_preview_image` renders fullsize when ready, else the thumb when ready (kicking the fullsize load), else the placeholder. LRU: keep an access-ordered list of ready image keys; evict past 48 entries (disk makes reloads cheap); cap decoded videos at 4 playlists.

### Files to Touch
`src/media.rs`: disk read-through, thumb-aware `PreviewImage`/render, LRU bound.
`src/app.rs`: prefetch hook on selection movement.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] `ImageLoadJob::run` reads the disk cache before HTTP and decodes via `spawn_blocking`.
- [x] Downloads still write the cache file on miss.
- [x] `PreviewImage` carries thumb + fullsize; overlay renders thumb while fullsize loads.
- [x] Prefetch of ±2 posts' thumbnails on selection movement, deduped by existing states.
- [x] LRU eviction beyond 48 decoded images; video cache capped at 4.
- [x] Tests: cache read-through (tempdir), thumb-first selection logic, LRU eviction order.
- [x] Gate: fmt, test, clippy -D warnings, build.

### Acceptance Criteria
**Scenario:** Second launch.
**GIVEN** images viewed in a previous run exist in the disk cache.
**WHEN** the user opens the media overlay offline.
**THEN** the image renders without any network request.

**Scenario:** Browsing toward a post with media.
**GIVEN** the selection moves to within two posts of an image post.
**WHEN** the user presses Space after a beat.
**THEN** the thumbnail is already rendered and fullsize replaces it when ready.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
Removed the dead ensure_item/ensure_images/ensure_url synchronous download path (no callers) rather than teaching it LRU. Corrupt disk-cache entries are deleted and fall through to the network. Known edge: if 48+ loads complete while the overlay shows an image that never gets re-rendered (touch happens on draw), the displayed entry could in principle be evicted and show 'Image queued' until the overlay reopens; render-time touch makes this effectively unreachable, documented here rather than complicating eviction. LRU/thumb tests use a Picker::halfblocks cache with a tempdir.
