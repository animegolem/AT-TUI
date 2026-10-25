---
node_id: AI-IMP-003-8
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - ui
  - navigation
  - scrolling
kanban_status: completed
depends_on:
  - AI-IMP-003-4
parent_epic: [[AI-EPIC-003-trustworthy-runtime-interaction-foundation]]
confidence_score: 0.94
date_created: 2026-08-19
date_completed: 2026-08-19
---

# AI-IMP-003-8-unified-view-buffer

## Profile content is pinned outside the scroll buffer
The profile summary is stored outside the view item list. The renderer always draws it first, subtracts its height, and scrolls only the posts below it. This makes one view behave like a fixed profile pane over a second post pane. View errors have a related problem: the renderer appends them after a viewport-sized list, so they do not participate in scrolling and can be clipped. Done state: profile and error content belong to one line-addressable view buffer and scroll naturally with posts.

### Out of Scope
Free-form mouse scrolling, selectable profile metadata, changes to pagination identity, removal of the fixed statusline, overlay layout changes, and a broad renderer module split.

### Design/Approach
Keep post selection and pagination indexed by `ViewItem`. Change `ViewState.scroll` from an item index to a rendered-line offset. Compose leading profile and error lines with rendered item lines before applying the viewport. Adjust the line offset to keep the selected post visible, but preserve offset zero when a profile first opens so readers see the document from its beginning. Moving back to the first post or jumping to the top restores the leading content.

### Files to Touch
`src/navigation.rs`: define line-offset scroll semantics and remove item-index assumptions.
`src/ui.rs`: compose one view buffer, apply the line viewport, and add regression tests.
`src/app.rs`: preserve top-of-feed behavior with the clarified scroll coordinate.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Treat `ViewState.scroll` as a rendered-line offset while keeping `selected` item-based.
- [x] Render the profile summary as leading buffer content instead of pinned content.
- [x] Render persistent view errors inside the same leading buffer.
- [x] Scroll wrapped profile lines off incrementally as selection moves down.
- [x] Restore leading content when navigation returns to the top.
- [x] Preserve thread selection, search, pagination, stack restoration, and pending-item behavior.
- [x] Add tests for profile scrolling, top restoration, narrow wrapped headers, and error placement.
- [x] Run the full validation gate.

### Acceptance Criteria
**Scenario:** A profile contains enough posts to exceed the viewport.
**GIVEN** the profile summary and posts render in one view.
**WHEN** the user moves down through the posts.
**THEN** profile lines scroll off the top incrementally instead of remaining pinned.

**Scenario:** The user returns to the top of a profile.
**GIVEN** the profile summary has scrolled away.
**WHEN** the user selects the first post or jumps to the top.
**THEN** the viewport returns to line zero and shows the profile summary again.

**Scenario:** A view contains a persistent error.
**GIVEN** the view also contains enough rows to fill the viewport.
**WHEN** the view renders from the top.
**THEN** the error appears inside the scrollable buffer instead of after an already full viewport.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
`ViewState.scroll` now represents a rendered-line offset. Selection remains an
item index, so post actions and pagination keep their existing identity model.
The renderer walks cached line slices and clones only visible lines; it does
not rebuild the complete document on each frame.

The full gate passed with 146 library tests, one CLI integration test,
formatting clean, Clippy warnings denied, an optimized release build, and no
whitespace errors. A saved-account PTY smoke test opened a live profile,
confirmed that moving from the first to the second post scrolled the profile
summary off, confirmed that moving back restored the summary, and exited
cleanly.

`src/app.rs` required no edit. Its only scroll-dependent top check already
treats offset zero as the top of the timeline under the new line semantics.
