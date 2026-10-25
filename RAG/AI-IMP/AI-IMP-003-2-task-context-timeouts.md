---
node_id: AI-IMP-003-2
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - async
  - lifecycle
  - reliability
kanban_status: completed
depends_on:
  - AI-IMP-003-1
parent_epic: [[AI-EPIC-003-trustworthy-runtime-interaction-foundation]]
confidence_score: 0.88
date_created: 2026-08-17
date_completed: 2026-08-17
---

# AI-IMP-003-2-task-context-timeouts

## Background completions have inconsistent ownership and no deadline
Several load events carry request IDs and some carry account DIDs, while write events carry neither. Account switching replaces the active client and navigation state without uniformly invalidating prior work. Requests also have no application-level deadline, so one hung future can leave a `pending_*` slot occupied indefinitely. Done state: every state-changing completion proves its request/account/generation ownership, obsolete work cannot mutate the app, and timeouts always release pending state.

### Out of Scope
Offline job persistence, cross-process queues, automatic replay of failed writes, and changing the single-owner app-state model.

### Design/Approach
Add a compact task context containing request ID, account DID where applicable, and an app/view generation. Centralize task spawning and completion validation instead of repeating partial checks in each event arm. Increment the relevant generation on account switches and state replacements; cancel invalidated tasks when handles are available and still reject late events defensively. Apply explicit deadlines by operation class and convert timeout into a normal completion error so pending state is cleared. Extract task/event lifecycle code from `app.rs` only as needed.

### Files to Touch
`src/app.rs`: task context creation, validation, timeout handling, account-switch invalidation.
`src/task.rs` or `src/app/tasks.rs`: new lifecycle types/helpers if the extraction remains narrow.
`src/lib.rs`: module export if needed.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Define task context and generation semantics for loads, polls, writes, and media work.
- [x] Make every state-changing `AppEvent` carry or resolve a valid context.
- [x] Include account identity on write completions and reject old-account results.
- [x] Invalidate outstanding account-scoped work when an account switch commits.
- [x] Add explicit deadlines for API operations and surface timeout as a recoverable event result.
- [x] Guarantee pending flags/counters clear after success, error, timeout, cancellation, and stale completion.
- [x] Prevent late events from replacing navigation state created after their request began.
- [x] Add tests for stale account writes, stale view loads, timeout recovery, and late completion after cancellation.
- [x] Perform a hands-on account-switch and navigation smoke check.
- [x] Run the full validation gate.

### Acceptance Criteria
**Scenario:** A write completes after switching accounts.
**GIVEN** account A begins a like and the user switches to account B before it completes.
**WHEN** account A's result arrives.
**THEN** account B's navigation and counters remain unchanged.

**Scenario:** A poll hangs.
**GIVEN** a feed refresh exceeds its configured deadline.
**WHEN** the timeout fires.
**THEN** the pending refresh is cleared, health state records the timeout, and a later scheduled poll can run.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
All spawned results now travel inside a `TaskEvent` carrying request ID, origin account DID, view generation, and scope. Invalidation is logical rather than `JoinHandle`-based: obsolete tasks may finish until the 30-second HTTP deadline, but their events are rejected centrally and clear only their own pending slot. Physical cancellation remains correctly owned by the media scheduler/video tickets, where child and worker handles will exist. The hands-on check launched the real TUI with images disabled, opened and returned from a thread, switched from `littledrummer` to `animegolem`, switched back to `littledrummer`, and exited cleanly; the original account remained active and no Bluesky content was written. No blockers or missing ticket-scoped tests remain.
