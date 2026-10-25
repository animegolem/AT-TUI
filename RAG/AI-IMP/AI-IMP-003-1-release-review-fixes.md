---
node_id: AI-IMP-003-1
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - reliability
  - security
kanban_status: completed
depends_on:
parent_epic: [[AI-EPIC-003-at-tui-release-review]]
confidence_score: 0.9
date_created: 2026-10-25
date_completed: 2026-10-25
---

# AI-IMP-003-1-release-review-fixes

## Summary of Issue #1
Fifteen defects found in review, headed by the session dying two hours after launch (refresh keyed on 401; the PDS returns `400 ExpiredToken`). Done state: each defect fixed with a test, the gate green, and the expiry, pagination and paste fixes verified against the live app.

### Out of Scope
Visual changes and any change to ratified keymap behaviour (see AI-IMP-003-2).

### Design/Approach
- `api.rs`: classify responses after reading the body (`access_token_expired`, `refresh_token_rejected`); typed `SessionExpired`; before giving up, adopt a refresh token another instance saved; shared `http_client()` with 10 s connect / 30 s total timeouts and a User-Agent.
- `navigation.rs`/`app.rs`: `ViewState.page_request` replaces the app-wide pagination slot; results find their view by request id.
- `app.rs`: bracketed paste routed to composer/search; global Ctrl-C; `is_web_uri` gate on `open_uri`; `saved_draft` restored per target; `PostCompleted` keeps the draft on failure; `feed_load_fallback` reverts a failed switch; `set_error_status` (8 s).
- `model.rs`: `apply_feed_display_names` (getFeedGenerators); reply participants plus viewer DID for `hideRepliesByUnfollowed`, defaulting to true as `@atproto/api` does.
- `ui.rs`: reserved error row; layout cache keyed on the minute; the run loop redraws once a minute.
- `config.rs`: temp-file + fsync + rename. `media.rs`: startup prune (30 days / 256 MiB), mtime bumped on cache hits. `main.rs`: strip leading `@` at login.

### Files to Touch
`src/api.rs`, `src/app.rs`, `src/navigation.rs`, `src/model.rs`, `src/ui.rs`, `src/config.rs`, `src/media.rs`, `src/main.rs`.

### Implementation Checklist
- [x] Expired-token refresh, SessionExpired, rotated-token adoption; tests incl. the live-captured body (24e318b).
- [x] HTTP timeouts on API and media clients (24e318b).
- [x] Per-view pagination; regression test for navigate-away; error row + test that fails on the old renderer (bb77fac).
- [x] Bracketed paste, Ctrl-C, web-only links, kept drafts; tests (7bfd7fa).
- [x] Feed names, reply filter semantics and default, failed-switch revert, fresh timestamps; tests (5517cc3).
- [x] Atomic config write, cache prune, login `@` strip; tests (56028f6).
- [x] Gate: `cargo fmt --check`, `cargo test` (131), `cargo clippy --all-targets -- -D warnings`.

### Acceptance Criteria
**GIVEN** a client holding a two-hour-old access token **WHEN** it requests unread notifications **THEN** it refreshes once, retries, succeeds and saves the rotated tokens. Verified live with a throwaway probe against the real store; `main` fails the same probe with `400 ExpiredToken`.

### Issues Encountered
The first after-fix probe run reused main's binary through the shared `~/.cargo-target` (the worktree and checkout uplift to the same path) and appeared to fail; a forced rebuild proved the fix. Filed to the house ledger. The ExpiredToken path has unit tests and one live proof but no HTTP-mock integration test. The reply-filter fix changes what the Following feed shows (on the owner's timeline, 9 of 23 replies were to people he doesn't follow); it matches the official client and is governed by Bluesky's feed preferences.
