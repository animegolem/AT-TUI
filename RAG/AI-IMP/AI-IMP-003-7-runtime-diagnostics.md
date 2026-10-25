---
node_id: AI-IMP-003-7
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - diagnostics
  - observability
  - reliability
kanban_status: planned
depends_on:
  - AI-IMP-003-1
  - AI-IMP-003-2
  - AI-IMP-003-5
  - AI-IMP-003-6
parent_epic: [[AI-EPIC-003-trustworthy-runtime-interaction-foundation]]
confidence_score: 0.86
date_created: 2026-08-17
date_completed:
---

# AI-IMP-003-7-runtime-diagnostics

## Runtime failures are visible only as transient symptoms
The app can show a generic offline segment, but it does not retain enough safe operational context to distinguish expired auth, a timed-out poll, stale cancellation, media saturation, or a failed playback handoff. This made the idle-refresh defect appear like login loss and allowed an incorrect HTTP 401 assumption to survive. Done state: a compact in-app diagnostics surface and optional sanitized log explain current runtime health without exposing credentials or private payloads.

### Out of Scope
Remote telemetry, crash reporting services, analytics, full request/response logging, and a general settings framework.

### Design/Approach
Maintain bounded diagnostic state owned by `App`: last successful feed or
notification poll, last safe XRPC status and code, last refresh outcome and
time, current and oldest task by class, timeout and cancellation counts, media
queue depth, active workers, cache hit and miss counts, and the last mpv
playback outcome. Render the state as a menu section or dedicated overlay.
Optionally write sanitized structured events to a bounded local log. Centralize
redaction and test forbidden fields.

### Files to Touch
`src/diagnostics.rs`: bounded state, safe event model, redaction, optional writer.
`src/app.rs`: record transport/task/media events and open diagnostics.
`src/ui.rs`: compact diagnostics rendering.
`src/main.rs`: opt-in log configuration if used.
`src/keymap.rs`: diagnostics binding/help entry.
`README.md`: diagnostics usage and privacy contract.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [ ] Define a bounded diagnostic state model with monotonic timestamps where appropriate.
- [ ] Record last poll success/failure and typed XRPC status/code.
- [ ] Record session refresh attempts/outcomes without recording tokens.
- [ ] Record pending task counts, oldest age, timeouts, cancellations, and stale drops.
- [ ] Record media queue, worker, cache, and mpv playback summaries.
- [ ] Render a compact diagnostics surface that works at narrow widths.
- [ ] Add opt-in bounded local logging only if redaction and lifecycle are testable.
- [ ] Add tests proving tokens, authorization headers, app passwords, and private bodies cannot enter diagnostic output.
- [ ] Run and document the 24-hour/two-expiry live idle validation required by the epic.
- [ ] Perform hands-on narrow/wide diagnostics rendering checks.
- [ ] Run the full validation gate.

### Acceptance Criteria
**Scenario:** Polling stops after an authentication response.
**GIVEN** a background poll receives an XRPC error.
**WHEN** the user opens diagnostics.
**THEN** it shows the safe XRPC code/status, last successful poll age, refresh outcome, and current pending-task state.
**AND** it contains no token or response payload.

**Scenario:** The epic is ready to close.
**GIVEN** all implementation tickets and automated gates pass.
**WHEN** AT-TUI is left running for at least 24 hours across at least two token expiries.
**THEN** feed and notification polling continue without restart and the observed evidence is recorded in this ticket or an AI-LOG.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
