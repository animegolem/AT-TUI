---
node_id: AI-IMP-005-3
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - reliability
  - performance
kanban_status: completed
depends_on:
  - AI-IMP-003-5
parent_epic: [[AI-EPIC-005-runtime-correctness-and-efficiency]]
confidence_score: 0.90
date_created: 2026-09-04
date_completed: 2026-09-04
---

# AI-IMP-005-3-visible-media-revival

## Visible requests revive cancelled image prefetch
MediaCache::prepare_image_load rejects loading URLs before MediaScheduler::submit can revive or promote their records. After scrolling cancels an in-flight prefetch, a visible request for the same URL is rejected and its eventual successful image is discarded. A deterministic App/cache/scheduler reproduction ends with missing instead of ready.

### Out of Scope
Broad module extraction, new UI features, account/auth redesign, dependency upgrades, and closing the separate 24-hour idle-validation or Ghostty acceptance work.

### Design/Approach
Governed by [[AI-EPIC-005-runtime-correctness-and-efficiency]] section 1. Separate scheduler priority/ownership admission from deciding whether a new image download is needed. Visible requests must promote queued or running speculation and revive cancelled in-flight work without duplicate downloads. Cancellation must still discard work that remains obsolete. Exercise the real App submission/completion boundary, not only the scheduler in isolation.

### Files to Touch
src/app.rs media submission/completion section; src/media.rs cache admission; src/media_scheduler.rs; focused tests.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Revive cancelled in-flight work when the same URL becomes visible.
- [x] Promote queued/in-flight speculation while maintaining deduplication and concurrency limits.
- [x] Retain discard behavior for obsolete unrequested work and bounded retry behavior.
- [x] Add integrated App/cache/scheduler regressions and run focused checks.

### Acceptance Criteria
**GIVEN** an in-flight thumbnail was cancelled on selection movement, **WHEN** that URL becomes visible before completion, **THEN** the existing successful result becomes ready with one request.

**GIVEN** cancelled media remains obsolete, **WHEN** it completes, **THEN** it is discarded and its physical worker slot is released.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
Accepted by the root reviewer at source commit 267206e. Cache admission now forwards renewed demand for loading URLs. The scheduler promotes queued and ordinary in-flight speculation, revives cancelled in-flight work, and renews generation without duplicating workers or resetting retry attempts.

Integrated App/cache/scheduler regressions cover promotion, revival, deduplication, obsolete-result discard, and bounded retries. Root independently reproduced the focused media checks and the final full gate at 4314ef1. No unresolved ticket-scoped issue. See [[2026-09-04-LOG-AI-runtime-cleanup-epic-005-complete]].
