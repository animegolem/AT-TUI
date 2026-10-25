---
node_id: AI-IMP-005-5
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - reliability
  - performance
kanban_status: completed
depends_on:
  - AI-IMP-005-2
  - AI-IMP-005-4
parent_epic: [[AI-EPIC-005-runtime-correctness-and-efficiency]]
confidence_score: 0.90
date_created: 2026-09-04
date_completed: 2026-09-04
---

# AI-IMP-005-5-remove-obsolete-helper-paths

## Remove unused helpers and test the production media loader
The app retains unused public helpers and duplicate old loading logic. Examples at 6be2bc3 are BskyClient::login, NavigationStack::breadcrumb, PreviewMedia::source_label, MediaCache::should_load/load_job, and HomeFeedPrefs::status_label. ImageLoadJob::run/load is used only by an old cache-hit test while production uses run_limited. Public exports hide these from ordinary dead-code linting.

### Out of Scope
Broad module extraction, new UI features, account/auth redesign, dependency upgrades, and closing the separate 24-hour idle-validation or Ghostty acceptance work.

### Design/Approach
Governed by [[AI-EPIC-005-runtime-correctness-and-efficiency]] section 1. Audit repository callers, tests, examples, and documentation before removal. Remove demonstrably unused internal convenience APIs and consolidate duplicate loading logic so tests cover the production limited path. Preserve purposeful test tools (including generated README keymap verification), runtime behaviors, account commands, and useful public entry points. This application has no promised library API; do not expand removal into unused parsed data fields or dependency upgrades. Record the exact removed symbols and caller evidence.

### Files to Touch
src/api.rs; src/media.rs; src/navigation.rs; src/model.rs; src/video_player.rs; tests only as justified.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Verify callers and distinguish obsolete runtime paths from meaningful test/documentation helpers.
- [x] Remove verified unused helpers and consolidate the duplicate image-loading path.
- [x] Route disk-hit/no-network coverage through the production limited loader.
- [x] Record symbol removals and run the full test/Clippy/documentation gate.

### Acceptance Criteria
**GIVEN** the obsolete helper path is removed, **WHEN** the full app/CLI/test/doc gates run, **THEN** the supported behavior remains intact and the disk-hit test executes the production image loader.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
Accepted by the root reviewer at source commit 13a872a. Repository callers and documentation were checked before removing 12 obsolete helpers: BskyClient::login; NavigationStack::breadcrumb; PreviewMedia::source_label; HomeFeedPrefs::status_label; MpvPlayer::executable; MediaCache::{should_load, mark_loading, load_job, should_load_url}; ImageLoadJob::{url, run, load}.

The disk-hit/no-network test now uses production run_limited with scheduler execution limits. Purposeful keymap/README helpers, preview_images, and source label methods with real callers remain. Owned byte buffers also remove two full download-buffer copies from publication/decode. No dependencies changed. Full test, Clippy, documentation, release, and package gates passed at 4314ef1. No unresolved ticket-scoped issue. See [[2026-09-04-LOG-AI-runtime-cleanup-epic-005-complete]].
