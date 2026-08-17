---
node_id: AI-EPIC-003
tags:
  - EPIC
  - AI
  - at-tui
  - reliability
  - runtime
  - keymap
  - media
date_created: 2026-08-17
date_completed:
kanban_status: planned
AI_IMP_spawned:
  - AI-IMP-003-1
  - AI-IMP-003-2
  - AI-IMP-003-3
  - AI-IMP-003-4
  - AI-IMP-003-5
  - AI-IMP-003-6
  - AI-IMP-003-7
---

# AI-EPIC-003-trustworthy-runtime-interaction-foundation

## Problem Statement/Feature Scope
AT-TUI is attractive and usable, but several lifecycle boundaries remain prototype-grade. A live access token expires with HTTP 400 `ExpiredToken`, while the client refreshes only on HTTP 401; polling therefore stops until restart even though the saved login remains valid. Network tasks have uneven stale-result guards, writes are not account-scoped, credential saves can replace unreadable configuration with an empty file, key behavior is split between one normal-mode action map and direct overlay matches, and speculative media work has no shared priority or concurrency policy. These are now the main obstacles to leaving the app open and extending it confidently.

## Proposed Solution(s)
Deliver seven behavior-led phases. Each phase must improve the running application and extract only the module seam needed to own that behavior.

**Phase 1 — correct XRPC transport semantics.** Parse structured XRPC errors, refresh on `ExpiredToken` regardless of its HTTP 400 status, retry once, and test the complete HTTP boundary.

**Phase 2 — bounded task lifecycle.** Give every background result a request/account/generation context, reject stale work uniformly, cancel invalidated work where practical, and ensure calls cannot occupy pending slots indefinitely.

**Phase 3 — durable session persistence.** Replace truncate-in-place credential writes with atomic account-scoped saves, preserve readable state on errors, and finish legacy-session migration safely.

**Phase 4 — contextual keymap registry.** Route timeline, menu, media, link-picker, composer, and confirmation keys through one binding registry that also supplies help text.

**Phase 5 — prioritized media scheduling.** Bound downloads and decodes, prioritize visible media over prefetch, cancel obsolete speculation, permit controlled retry, and bound the disk cache.

**Phase 6 — progressive video delivery.** Stream decoded frame batches to the UI instead of waiting for the complete ffmpeg run, with bounded memory and clean cancellation.

**Phase 7 — runtime diagnostics.** Add a compact diagnostics surface and optional sanitized log for poll health, XRPC failures, task age, session refresh, and media-cache behavior.

## Path(s) Not Taken
This epic does not add OAuth, DMs, global search, moderation features, image upload, wide side panes, theming, or user-editable keymap files. It does not replace all `serde_json::Value` parsing or rewrite the application into a framework. Inline timeline images remain a later product decision after media work is bounded. Audio remains outside the progressive-video ticket and may follow the existing video/audio brief.

## Success Metrics
- A live app runs for at least 24 hours and crosses at least two access-token expiries while timeline and notification polling continue without restart.
- An HTTP-boundary test proves HTTP 400 `ExpiredToken` refreshes once and retries successfully; unrelated HTTP 400 responses do not refresh.
- Every tracked background operation either completes, times out, or is invalidated; no pending slot remains occupied indefinitely.
- Results created for an old account or view generation cannot mutate current state.
- Simulated interrupted or malformed session-store writes preserve the last readable account configuration.
- All interactive contexts dispatch through the binding registry, and rendered help matches the registered bindings.
- Media concurrency and disk use remain within tested bounds; selected media overtakes speculative prefetch.
- Existing gates remain green: formatting, tests, Clippy with warnings denied, build, and `git diff --check`.

## Requirements

### Functional Requirements
- [ ] FR-1: The transport shall parse XRPC error code, message, and HTTP status into a typed error.
- [ ] FR-2: `ExpiredToken` shall trigger one single-flight refresh and one retry for both reads and writes.
- [ ] FR-3: Non-authentication HTTP 400 responses shall not spend a refresh token.
- [ ] FR-4: HTTP-level regression tests shall cover expiry, successful retry, failed refresh, and non-authentication errors.
- [ ] FR-5: Every app task result shall carry enough context to prove it belongs to the active request, account, and view generation.
- [ ] FR-6: Account switches shall invalidate or reject all prior account-scoped completions, including writes.
- [ ] FR-7: Network and media operations shall have explicit deadlines and leave pending state recoverable after timeout.
- [ ] FR-8: Account configuration writes shall be atomic and shall never treat a read/parse error as an empty configuration.
- [ ] FR-9: Successful legacy-session migration shall not leave a stale credential source that can later be re-imported.
- [ ] FR-10: Normal and overlay key handling shall dispatch through one contextual binding registry.
- [ ] FR-11: Key help shall derive from the same binding metadata used for dispatch.
- [ ] FR-12: Media work shall use bounded, priority-aware scheduling with deduplication and obsolete-prefetch cancellation.
- [ ] FR-13: Failed media entries shall support an explicit or policy-bounded retry path.
- [ ] FR-14: The on-disk media cache shall have a tested cleanup bound.
- [ ] FR-15: Video decoding shall report progressive frame batches and enforce memory/frame limits.
- [ ] FR-16: Closing or replacing a video shall stop its decoder and ignore late frames.
- [ ] FR-17: Diagnostics shall expose sanitized poll, refresh, task, and media health without revealing tokens or private response bodies.
- [ ] FR-18: A live idle validation shall be recorded before this epic is completed.

### Non-Functional Requirements
- Preserve the narrow, single-column, keyboard-first product contract.
- Preserve single-owner application state; background work reports through events and cannot directly mutate `App`.
- Never log access tokens, refresh tokens, app passwords, authorization headers, or full private API payloads.
- Keep queues, task counts, decoded frames, memory caches, and disk caches explicitly bounded.
- Prefer behavior-led module extraction over a broad rewrite: transport, task context, keymap, and media scheduler become seams as their tickets land.
- Each ticket lands as an independently reviewable change with focused tests and a hands-on TUI check for user-visible behavior.

## Implementation Breakdown
- [ ] [[AI-IMP-003-1-xrpc-expired-token-retry]]: typed XRPC errors, HTTP 400 `ExpiredToken` refresh/retry, HTTP-boundary tests (FR-1..4).
- [ ] [[AI-IMP-003-2-task-context-timeouts]]: uniform task context, stale completion rejection, cancellation, deadlines (FR-5..7).
- [ ] [[AI-IMP-003-3-atomic-session-persistence]]: atomic account saves and safe legacy migration (FR-8..9).
- [ ] [[AI-IMP-003-4-contextual-keymap-registry]]: contextual dispatch and generated help (FR-10..11).
- [ ] [[AI-IMP-003-5-prioritized-media-scheduler]]: bounded priority queue, retry, cancellation, disk-cache lifecycle (FR-12..14).
- [ ] [[AI-IMP-003-6-progressive-video-events]]: progressive ffmpeg frame delivery and bounded cancellation (FR-15..16).
- [ ] [[AI-IMP-003-7-runtime-diagnostics]]: diagnostics surface, sanitized logging, live idle record (FR-17..18).

Dependency order: 003-1 first. 003-2 depends on 003-1. 003-3 may proceed after 003-1 with care around `api.rs`. 003-4 depends on 003-2 because both restructure `app.rs`. 003-5 depends on 003-2. 003-6 depends on 003-5. 003-7 closes the epic after 003-1, 003-2, 003-5, and 003-6 provide the signals it reports.
