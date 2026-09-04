---
node_id: AI-IMP-005-4
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - reliability
  - performance
kanban_status: planned
depends_on:
  - AI-IMP-005-3
parent_epic: [[AI-EPIC-005-runtime-correctness-and-efficiency]]
confidence_score: 0.90
date_created: 2026-09-04
date_completed:
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

- [ ] Enforce byte and age bounds after maintenance even when all entries were touched this session.
- [ ] Handle oversized files, concurrent readers/writers, corrupt entries, and maintenance failure without failing image display.
- [ ] Move blocking writes off async workers and remove scan waits from the image result path.
- [ ] Coalesce scans under bursts without losing the final cleanup; test maintenance lifecycle.
- [ ] Record boundedness and scan-count evidence with deterministic temporary-directory tests.

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
Awaiting implementation review. The lead records acceptance and final evidence after reproducing validation.
