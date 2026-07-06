---
node_id: AI-IMP-002-7
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - ui
  - polish
kanban_status: completed
depends_on: [[AI-IMP-002-2]]
parent_epic: [[AI-EPIC-002-at-tui-stabilization-daily-driver]]
confidence_score: 0.9
date_created: 2026-07-06
date_completed: 2026-07-06
---

# AI-IMP-002-7-visual-quick-wins

## Summary of Issue #1
Two cheap visual upgrades from the review: the pending-task indicator is a static `…` that gives no sense of life, and quote posts render with an ASCII `+-- quote` / `| ` frame that clashes with the rounded borders everywhere else. Done state: a braille spinner animates in the statusline while any task is pending, and quotes render in box-drawing frames (`╭─ ❞ …` / `│ …` / `╰─`).

### Out of Scope
Author identity colors, gutter-bar selection, inline facet styling, inline timeline thumbnails — the larger polish items stay in the backlog for the next visual pass.

### Design/Approach
Spinner: `⠋⠙⠹⠸⠼⠴⠦⠧` indexed by a frame counter the main loop advances on a ~120 ms cadence while `has_pending_tasks()` (002-2's dirty flag already keeps redraws flowing then); replaces the `…` segment content. Quote frames: `render_quote_lines` emits `╭─ ❞ {author} @{handle} {time}` as the header, prefixes body/media lines with `│ `, and closes with `╰─`, colors unchanged. Update the affected ui tests.

### Files to Touch
`src/ui.rs`: spinner segment, quote frame rendering, tests.
`src/app.rs`: spinner frame counter advanced with pending tasks.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Spinner frames cycle in the pending segment while tasks run; static state gone.
- [x] Quote posts render with `╭─`/`│`/`╰─` box-drawing frame and `❞` marker.
- [x] Nested-quote and media summary lines align inside the frame.
- [x] Tests updated for the new quote frame text and spinner presence.
- [x] Gate: fmt, test, clippy -D warnings, build.

### Acceptance Criteria
**Scenario:** Background load in flight.
**GIVEN** a feed refresh is pending.
**WHEN** the statusline renders across successive frames.
**THEN** the pending segment shows successive braille spinner characters.

**Scenario:** Quote post in the timeline.
**GIVEN** a post quoting another post.
**WHEN** it renders.
**THEN** the quote appears inside a `╭─`/`│`/`╰─` frame with the author header on the top edge.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
Spinner cadence is 120ms gated on has_pending_tasks; the dirty-flag loop from 002-2 already redraws while tasks pend so no extra wakeups were added. Minor known inconsistency: media-summary rows inside a quote frame render their │ prefix in the summary color rather than frame yellow; left as-is to keep push_wrapped_summary generic.
