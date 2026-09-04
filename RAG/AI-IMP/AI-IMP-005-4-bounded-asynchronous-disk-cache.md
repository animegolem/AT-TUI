---
node_id: AI-IMP-005-4
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - reliability
  - performance
kanban_status: completed
depends_on:
  - AI-IMP-005-3
parent_epic: [[AI-EPIC-005-runtime-correctness-and-efficiency]]
confidence_score: 0.90
date_created: 2026-09-04
date_completed: 2026-09-04
---

# AI-IMP-005-4-bounded-asynchronous-disk-cache

## Bound session disk use and remove cleanup from image latency
Every disk entry read/written during the process is pinned until exit; the 256 MiB target can therefore be exceeded indefinitely. A small reproduction retained 12 bytes against an 8-byte target. Each download also performs a synchronous file write and awaits a complete cache scan before decoding. This deliberately revises the current-session protection policy in AI-IMP-003-5.

### Out of Scope
Broad module extraction, new UI features, account/auth redesign, dependency upgrades, and closing the separate 24-hour idle-validation or Ghostty acceptance work.

### Design/Approach
Governed by [[AI-EPIC-005-runtime-correctness-and-efficiency]] section 1. Replace lifetime pins with the minimum protection required for active I/O, or eliminate pins when images/bytes are owned in memory. At quiescence after successful maintenance, the cache must satisfy its byte/age policy, including files touched in this session and oversized entries. Use atomic cache publication where concurrent maintenance can observe writes. Offload blocking disk operations and batch/coalesce maintenance; image decode/display must not await a directory scan. Ensure a final scheduled cleanup occurs after a download burst, and keep failures best-effort. Preserve the existing cache directory and format; no cache migration.

### Files to Touch
src/media.rs; src/media_scheduler.rs only if needed for lifecycle integration; focused temporary-directory tests.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Enforce byte and age bounds after maintenance even when all entries were touched this session.
- [x] Handle oversized files, concurrent readers/writers, corrupt entries, and maintenance failure without failing image display.
- [x] Move blocking writes off async workers and remove scan waits from the image result path.
- [x] Coalesce scans under bursts without losing the final cleanup; test maintenance lifecycle.
- [x] Record boundedness and scan-count evidence with deterministic temporary-directory tests.

### Acceptance Criteria
**GIVEN** a session downloads more than the configured cache budget, **WHEN** writes and successful scheduled maintenance finish, **THEN** retained cache bytes are within budget without restarting.

**GIVEN** several images finish together, **WHEN** cleanup is slow, **THEN** image results can complete independently and scans are coalesced rather than launched once per file.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
Accepted by the root reviewer at source commit eeb6098. Atomic publication runs on the blocking pool within bounded media jobs; a coalesced maintenance worker handles scans independently. Generation tracking under the worker-state lock ensures another pass after requests arrive during a scan. The synchronous cache constructor does not require a Tokio runtime.

Lifetime final-file pins are removed. Only active in-process staging is protected; unknown/inactive staging is eligible for immediate cleanup. A second process may therefore lose a best-effort cache publication, but display still decodes owned bytes. Bounds apply after successful quiescent maintenance; I/O failure remains nonfatal and a later publication can request maintenance again.

Deterministic barrier tests completed 12 publications totaling 72 bytes while scanning was paused; two passes left 6 bytes under an 8-byte budget. A publication after idle caused a third pass. The real production loader completed while maintenance was blocked. Tests also cover oversize/age policy, corrupt entries, staging, failed publication/maintenance, startup, and concurrent I/O. Full gate passed at 4314ef1; no unresolved ticket-scoped issue. See [[2026-09-04-LOG-AI-runtime-cleanup-epic-005-complete]].
