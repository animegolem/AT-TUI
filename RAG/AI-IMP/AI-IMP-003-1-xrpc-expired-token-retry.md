---
node_id: AI-IMP-003-1
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - auth
  - xrpc
  - reliability
kanban_status: planned
depends_on:
parent_epic: [[AI-EPIC-003-trustworthy-runtime-interaction-foundation]]
confidence_score: 0.95
date_created: 2026-08-17
date_completed:
---

# AI-IMP-003-1-xrpc-expired-token-retry

## Expired access tokens do not trigger the refresh path
Live evidence on 2026-08-17 showed `app.bsky.feed.getTimeline` returning HTTP 400 with `{"error":"ExpiredToken","message":"Token has expired"}` for an expired access JWT. `BskyClient` retries only HTTP 401, so the running app stops polling until restart calls `refresh_session` unconditionally. Done state: transport code understands structured XRPC errors, refreshes once on `ExpiredToken`, retries the original request once, and proves the behavior at an actual HTTP boundary.

### Out of Scope
OAuth, proactive refresh based on decoded JWT expiry, general exponential retry, app task timeouts, and UI diagnostics.

### Design/Approach
Introduce a typed XRPC failure containing HTTP status plus optional `error` and `message` fields. Centralize authenticated send/parse/retry behavior so GET and POST cannot diverge. Treat XRPC code `ExpiredToken` as the refresh signal regardless of HTTP 400/401; retain HTTP 401 compatibility only when it is demonstrably an authentication failure. Use the existing shared session and single-flight refresh gate. Buffer request metadata/body only as needed for exactly one retry. Add a local mock HTTP server or comparably realistic harness that exercises response status and JSON bodies through `reqwest`.

### Files to Touch
`src/api.rs`: typed XRPC error, common authenticated request/retry path, tests.
`Cargo.toml` / `Cargo.lock`: only if a focused dev-only HTTP test dependency is justified.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [ ] Add a typed XRPC error representation that retains status, error code, and safe message.
- [ ] Refactor response parsing so callers can classify an error before it becomes a string.
- [ ] Route authenticated GET and POST through one refresh-and-retry policy.
- [ ] Refresh on `ExpiredToken` and retry the original operation exactly once.
- [ ] Preserve the existing single-flight guard for concurrent expiries.
- [ ] Ensure generic HTTP 400 responses do not trigger refresh.
- [ ] Ensure a failed refresh or failed retry returns the typed causal error without looping.
- [ ] Add HTTP-boundary tests for 400 `ExpiredToken`, successful refresh/retry, unrelated 400, and failed refresh.
- [ ] Run formatting, tests, Clippy with warnings denied, build, and `git diff --check`.

### Acceptance Criteria
**Scenario:** Access token expires while AT-TUI is running.
**GIVEN** a timeline request receives HTTP 400 with XRPC code `ExpiredToken`.
**WHEN** the authenticated request policy handles the response.
**THEN** one session refresh occurs and the original timeline request retries once with the new access token.
**AND** later client clones observe the rotated session.

**Scenario:** A request is invalid for a non-authentication reason.
**GIVEN** a write receives HTTP 400 with a code other than `ExpiredToken`.
**WHEN** the response is handled.
**THEN** no refresh token is spent and the structured failure reaches the caller.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
