---
node_id: AI-IMP-002-1
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - auth
  - reliability
kanban_status: planned
depends_on:
parent_epic: [[AI-EPIC-002-at-tui-stabilization-daily-driver]]
confidence_score: 0.9
date_created: 2026-07-06
date_completed:
---

# AI-IMP-002-1-session-sharing-failure-visibility

## Summary of Issue #1
Every background task clones `BskyClient` including its JWTs by value. When the access token expires (~2h), the clone that hits the 401 refreshes, saves to disk, and is dropped; the main client keeps the old tokens, whose refresh JWT ATProto has now revoked. Every later request 401s and fails to re-refresh: the app stops updating until restart. Failures are also invisible — refresh errors flash a 2-second status and a failed unread poll zeroes the badge. Done state: all clones share one live session, concurrent 401s trigger exactly one refresh, poll failures surface a persistent statusline indicator, and the badge holds its last value on error.

### Out of Scope
OAuth, token expiry prediction (proactive refresh), retry backoff tuning, and any keymap or UI work beyond the offline indicator.

### Design/Approach
Wrap the session in `Arc<std::sync::RwLock<Session>>` inside `BskyClient` — reads are brief and never held across an await. Single-flight refresh via `Arc<tokio::sync::Mutex<()>>`: capture the current access JWT, acquire the gate, and if the JWT changed while waiting another task already refreshed — skip. Otherwise call `refreshSession` with the current refresh JWT, write the new tokens under the lock, and persist via the store. `session()` (returning `&Session`) becomes `session_snapshot()` returning a clone; hot call sites that only need identity use new `did()`/`handle()` helpers. In `App`, add `consecutive_poll_failures: u32` — incremented when the feed refresh check or unread poll errors, reset on success; the statusline shows a persistent `⚠ offline` segment when ≥ 2. On unread poll error, keep the previous badge value.

### Files to Touch
`src/api.rs`: shared session, single-flight refresh, accessor changes.
`src/app.rs`: call-site updates, failure counter, badge retention.
`src/ui.rs`: offline statusline segment.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [ ] `api.rs`: store `session: Arc<RwLock<Session>>` and `refresh_gate: Arc<tokio::sync::Mutex<()>>`; `Clone` shares both.
- [ ] `api.rs`: `send_get`/`send_post` read the access JWT through the lock per request.
- [ ] `api.rs`: `refresh_session` is single-flight with a skip when another task refreshed while waiting.
- [ ] `api.rs`: replace `session()` with `session_snapshot()` + `did()`/`handle()`; update all call sites.
- [ ] `app.rs`: add `consecutive_poll_failures`, increment on refresh-check and unread-poll errors, reset on success.
- [ ] `app.rs`: failed unread poll no longer zeroes `unread_notifications`.
- [ ] `ui.rs`: persistent `⚠ offline` segment while `consecutive_poll_failures >= 2`.
- [ ] Tests: single-flight skip logic (JWT-changed short-circuit), badge retention on error, offline segment rendering.
- [ ] Gate: `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo build`.

### Acceptance Criteria
**Scenario:** Access token expires while the app is idle.
**GIVEN** a running app whose access JWT has expired and two background polls fire concurrently.
**WHEN** both receive 401 and attempt refresh.
**THEN** exactly one `refreshSession` call is made and both retries succeed with the new token.
**AND** the main client's next request uses the refreshed JWT without restarting.

**Scenario:** Network drops.
**GIVEN** the machine loses connectivity.
**WHEN** two consecutive polls fail.
**THEN** the statusline shows a persistent offline indicator and the unread badge keeps its last known value.
**AND** the indicator clears on the next successful poll.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
