---
node_id: AI-IMP-005-2
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - reliability
  - performance
kanban_status: completed
depends_on:
  - AI-IMP-005-1
parent_epic: [[AI-EPIC-005-runtime-correctness-and-efficiency]]
confidence_score: 0.90
date_created: 2026-09-04
date_completed: 2026-09-04
---

# AI-IMP-005-2-pagination-owner-cleanup

## Pagination releases the view that owns the request
Stale pagination cleanup writes current_mut().loading=false. If a thread opens before timeline pagination completes, the timeline retains loading=true and cannot load another page after returning. The existing single pending slot can also be superseded by another view. A deterministic late-page reproduction confirms the stuck flag.

### Out of Scope
Broad module extraction, new UI features, account/auth redesign, dependency upgrades, and closing the separate 24-hour idle-validation or Ghostty acceptance work.

### Design/Approach
Governed by [[AI-EPIC-005-runtime-correctness-and-efficiency]] section 1. Use 005-1 ownership to apply or discard each page result and release only its originating request/view. Valid pages may update a retained background view; obsolete pages must release their own state without clearing a newer request. A popped or replaced target must not mutate a surviving view. Keep per-view cursor and error semantics coherent.

### Files to Touch
src/app.rs; src/navigation.rs; focused tests in those modules.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Release origin loading state after success, failure, stale completion, and invalidation.
- [x] Handle retained background views and popped/replaced views without cross-view mutations.
- [x] Prove an old completion cannot clear a newer pagination request.
- [x] Prove pagination can resume when returning to a retained view; run focused checks.

### Acceptance Criteria
**GIVEN** timeline pagination is pending and a thread is opened, **WHEN** pagination finishes and the user returns, **THEN** the timeline can load another page and retains coherent cursor/selection/scroll state.

**GIVEN** a newer request owns a view, **WHEN** an older result arrives, **THEN** the newer loading state remains intact.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
Accepted by the root reviewer at source commit 599f170. Pagination tracks tokens per view and releases only the matching owner/request. Tests cover retained views, concurrent completions in both orders, failures, popped/replaced targets, newer tokens, and full reload invalidation.

Review also found that profile/notification Reload could immediately enqueue pagination using the outgoing cursor in after_input. A request-scoped foreground reload token now suppresses that page until the reload completes or navigation invalidates it; regression coverage exercises the event-loop follow-up. The root's baseline PTY fixture reproduced a stuck Following [loading] view; the final binary clears it and resumes pagination while preserving navigation. Full gate passed at 4314ef1; no unresolved ticket-scoped issue. See [[2026-09-04-LOG-AI-runtime-cleanup-epic-005-complete]].
