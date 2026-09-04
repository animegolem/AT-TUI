---
node_id: AI-IMP-005-6
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - reliability
  - performance
kanban_status: completed
depends_on:
  - AI-IMP-005-2
  - AI-IMP-005-5
parent_epic: [[AI-EPIC-005-runtime-correctness-and-efficiency]]
confidence_score: 0.90
date_created: 2026-09-04
date_completed: 2026-09-04
---

# AI-IMP-005-6-incremental-layout-and-redraws

## Avoid redundant redraws and rebuild only appended row layouts
run_tui forces dirty on every loop while any task is pending, even though advance_spinner already exposes timed changes. UI cache append invalidation rebuilds selected and unselected text for every accumulated row, and scrolling sums preceding row heights repeatedly. These are concrete avoidable operations; no end-user speedup has been measured yet.

### Out of Scope
Broad module extraction, new UI features, account/auth redesign, dependency upgrades, and closing the separate 24-hour idle-validation or Ghostty acceptance work.

### Design/Approach
Governed by [[AI-EPIC-005-runtime-correctness-and-efficiency]] section 1. Draw on actual state changes, pending-state transitions, spinner ticks, status expiration, resize, and input. Reuse same-width cached rows on append, and maintain enough line-offset metadata to avoid walking all preceding rows just to find the viewport. Keep full invalidation on width/content/style changes where needed. Preserve selected and unselected styles and existing line-based viewport semantics; do not silently cap posts, reset selection, change timestamp policy, or redesign rendering. Measure operation counts or a reproducible local comparison before claiming improvement.

### Files to Touch
src/app.rs event-loop dirty handling; src/navigation.rs layout cache lifecycle; src/ui.rs layout/viewport code; focused tests.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Remove unconditional pending-task redraws while showing starts, completions, spinners, and status expiry.
- [x] Reuse unchanged row layouts on append; invalidate correctly for replacements, mutation, width, and removals.
- [x] Avoid linear scans of preceding cached rows for deep-view positioning.
- [x] Add render-buffer equivalence and cache-work regressions, including narrow/profile/error views.
- [x] Record measured work reduction and run full tests/Clippy/build plus a synthetic terminal rendering smoke.

### Acceptance Criteria
**GIVEN** a long rendered view receives one page at the same width, **WHEN** it draws, **THEN** only new rows require layout and existing row output/selection/viewport remain stable.

**GIVEN** a pending task with no input or visible state change, **WHEN** the loop runs between spinner ticks, **THEN** it does not redraw; task start/completion still redraw promptly.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
Accepted by the root reviewer at source commit 4314ef1. Timed visible-state snapshots replace unconditional pending-task redraws. Prefix line offsets and incremental row caching reuse unchanged layouts and locate the viewport without scanning every preceding row; selection-dependent height differences are accounted for.

Deterministic work counts: appending 50 posts to 2,000 cached posts builds 50 row pairs instead of 2,050; selection changes build zero. Twenty unchanged pending-state checks between spinner ticks request zero redraws, with transitions/ticks/expiry still covered. This is operation-count evidence, not a measured end-user timing claim.

Styled lines and complete Ratatui buffers match an independent fresh/linear rendering oracle through append, selection, mutation, 12/80-column resize, profile/error/notification/empty views, prepend, replace, and removal. Notification mutation now explicitly invalidates the layout. The existing timestamp-cache policy remains; removing incidental refresh on append was reviewed and accepted without adding a timer. Full gate and final 80x28/42x18 release-binary PTY smoke passed, including narrow-profile scroll restoration and zero idle terminal writes. No unresolved ticket-scoped issue. See [[2026-09-04-LOG-AI-runtime-cleanup-epic-005-complete]].
