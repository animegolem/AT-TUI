---
node_id: AI-EPIC-005
tags:
  - EPIC
  - AI
  - at-tui
  - reliability
  - performance
date_created: 2026-09-04
date_completed:
kanban_status: in-progress
AI_IMP_spawned:
  - AI-IMP-005-1
  - AI-IMP-005-2
  - AI-IMP-005-3
  - AI-IMP-005-4
  - AI-IMP-005-5
  - AI-IMP-005-6
---

# AI-EPIC-005-runtime-correctness-and-efficiency

## Problem Statement/Feature Scope
The September 4 review found three reproducible interaction bugs: late thread refresh can overwrite the timeline after Back, stale pagination can strand the original view as loading, and cancelled prefetch can discard an image now requested for display. Long-running sessions can also exceed the disk-cache target and accumulate avoidable layout work. Unused convenience APIs and a duplicate test-only media-loading path obscure the active implementation.

## Proposed Solution(s)
Deliver six small, reviewed changes that preserve the existing interface while making asynchronous work belong to the right view, visible media recover correctly, disk cache remain bounded, and rendering reuse completed work. Use isolated implementation clones, a source-grounded pre-implementation review, atomic ticket commits, and lead acceptance before merging.

## 1. Change Contract
This section governs this epic under AGENTS.md; it does not replace other project requirements. Revision 1, 2026-09-04.

- Preserve single-column keyboard-first behavior, selection, line-based scrolling, and the owner's reading position. Background completions must not redirect navigation or reset another view's viewport.
- Requests identify their owner and may only update that owner's current state. Foreground navigation intent is invalidated on Back/pop. Root-directed loads remain independent when their root target remains valid. Old completions must not clear newer pending work.
- Media stays bounded and deduplicated. A visible request can revive/promote existing work, while work that remains obsolete is discarded. Cache I/O failure is non-fatal to image display.
- Disk bounds apply after successful quiescent maintenance, including current-session entries. Active writes may temporarily exceed the target; the final maintenance after a burst must run. Lifetime session pins are replaced, deliberately refining AI-IMP-003-5.
- Rendering preserves output while reducing demonstrably redundant work. No post eviction, UI redesign, timestamp policy change, or broad architecture migration belongs in this epic.
- Remove helpers only after checking real callers. Keep meaningful test/documentation tools and validate the real production loader.
- The lead owns ticket/design rulings, integration, final review, and merges. Implementation agents use isolated clones, do not write to the owner's checkout, and do not push or perform destructive Git cleanup.

## Path(s) Not Taken
No feature expansion, OAuth, diagnostics implementation, framework rewrite, dependency upgrade, or arbitrary splitting of large modules. Existing AI-EPIC-003 live idle/terminal acceptance and AI-EPIC-004 hosted CI/release work remain separate. This epic fixes gaps in 003-2/003-5 and improves the redraw policy from 002-2 without claiming those older acceptance gates are complete.

## Success Metrics
Before this epic closes, focused regression tests must prove all three interaction bugs repaired; cache tests must enforce quiescent bounds and coalesced maintenance; layout tests must demonstrate reuse with equivalent output. The integration tip must pass formatting, all-target tests, Clippy with warnings denied, documentation, package verification, release build, and whitespace checks. Report local test counts and synthetic terminal validation separately from live owner/hosted CI evidence.

## Requirements

### Functional Requirements
- [ ] FR-1: Back and other navigation changes reject or correctly route late view completions.
- [ ] FR-2: Pagination releases its actual owner and cannot clear a newer request.
- [ ] FR-3: Visible image requests revive/promote existing work without duplicate downloads.
- [ ] FR-4: Cache maintenance bounds current-session disk use and leaves image completion independent of full scans.
- [ ] FR-5: Obsolete helpers are removed and production media loading receives relevant test coverage.
- [ ] FR-6: Redraws follow visible changes and appended rows reuse existing layouts.

### Non-Functional Requirements
Use deterministic tests and local mock/synthetic data for race validation. Do not send Bluesky writes or rotate saved account state for testing. Keep source changes within assigned fences. Review report assertions against actual code and reproduce counts. Completion means reviewed implementation and passing ticket-scoped acceptance, not dispatch or an agent's self-report.

## Implementation Breakdown
- [ ] [[AI-IMP-005-1-navigation-completion-ownership]]: Back navigation rejects late view completions.
- [ ] [[AI-IMP-005-2-pagination-owner-cleanup]]: Pagination releases the view that owns the request.
- [ ] [[AI-IMP-005-3-visible-media-revival]]: Visible requests revive cancelled image prefetch.
- [ ] [[AI-IMP-005-4-bounded-asynchronous-disk-cache]]: Bound session disk use and remove cleanup from image latency.
- [ ] [[AI-IMP-005-5-remove-obsolete-helper-paths]]: Remove unused helpers and test the production media loader.
- [ ] [[AI-IMP-005-6-incremental-layout-and-redraws]]: Avoid redundant redraws and rebuild only appended row layouts.

Dependency waves: 005-1 → 005-2 and 005-3 → 005-4 can run independently; 005-5 follows both; 005-6 follows 005-5. Ticket IDs 005-1 through 005-6 are reserved. Integration begins from 6be2bc3 on codex/ci-baseline and returns to that branch after review; publication/release is outside this sitting.
