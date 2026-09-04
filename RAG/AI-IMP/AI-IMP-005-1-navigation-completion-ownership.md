---
node_id: AI-IMP-005-1
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - reliability
  - performance
kanban_status: planned
depends_on:
  - AI-IMP-003-2
parent_epic: [[AI-EPIC-005-runtime-correctness-and-efficiency]]
confidence_score: 0.90
date_created: 2026-09-04
date_completed:
---

# AI-IMP-005-1-navigation-completion-ownership

## Back navigation rejects late view completions
At 6be2bc3, Back/BackOrQuit/Escape pop navigation without changing view generation. A thread reload result is then accepted and replaces whatever view is current. A deterministic reproduction refreshed a thread, returned to the timeline, delivered the old result, and observed thread posts replacing timeline posts. This is a gap in AI-IMP-003-2, not a new task framework.

### Out of Scope
Broad module extraction, new UI features, account/auth redesign, dependency upgrades, and closing the separate 24-hour idle-validation or Ghostty acceptance work.

### Design/Approach
Governed by [[AI-EPIC-005-runtime-correctness-and-efficiency]] section 1. Give view work explicit origin/target ownership and invalidate foreground navigation intent on every successful Back/pop. Completion must verify that its target still exists and that the request is current for that target; a reload must never mutate a different view. Preserve root-directed feed loads while nested where still valid. Establish the ownership seam used by 005-2. Account writes and global media retain their existing scopes.

### Files to Touch
src/app.rs; src/navigation.rs; focused tests in those modules.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [ ] Cover q, Back, and Escape while a thread reload or navigation load is pending.
- [ ] Reject late results after pop, replacement, and same-kind/different-view navigation.
- [ ] Preserve legitimate root feed loading under nested views and account/global task behavior.
- [ ] Run focused regression tests and formatting/Clippy checks.

### Acceptance Criteria
**GIVEN** a thread refresh is pending, **WHEN** the user returns to the timeline before completion, **THEN** the response cannot change timeline items, selection, scroll, or view kind.

**GIVEN** a root feed load remains valid under a pushed view, **WHEN** it completes, **THEN** the pushed view remains intact and the root receives its own result.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
Awaiting implementation review. The lead records acceptance and final evidence after reproducing validation.
