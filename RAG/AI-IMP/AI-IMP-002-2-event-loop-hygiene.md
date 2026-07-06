---
node_id: AI-IMP-002-2
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - input
  - performance
kanban_status: completed
depends_on:
parent_epic: [[AI-EPIC-002-at-tui-stabilization-daily-driver]]
confidence_score: 0.88
date_created: 2026-07-06
date_completed: 2026-07-06
---

# AI-IMP-002-2-event-loop-hygiene

## Summary of Issue #1
Three input/render defects: (1) key events are not filtered to `KeyEventKind::Press`, so kitty-protocol terminals and Windows fire every action twice; (2) `EnableMouseCapture` is on but no mouse event is handled — wheel does nothing while native text selection is blocked; (3) the loop redraws unconditionally at ~20 fps forever, burning CPU while idle. Done state: one action per keypress everywhere, wheel scrolls the selection, and the terminal redraws only when state changed.

### Out of Scope
Mouse clicks/hit-testing, configurable frame rates, and any keymap changes (AI-IMP-002-4).

### Design/Approach
Guard `handle_key` entry on `key.kind == KeyEventKind::Press`. Match `Event::Mouse` in `run_tui`: `ScrollUp`/`ScrollDown` map to move up/down on the current view (three lines each per wheel tick is unnecessary — one selection step per event is fine). Dirty-flag rendering: draw when (a) an input event was handled, (b) `drain_events` applied at least one event, (c) a video frame advanced, (d) the transient status crossed its expiry (track last visibility), or (e) `Event::Resize` arrived. While `has_pending_tasks()` is true stay dirty so progress/spinner segments animate. Poll timeout stays 50 ms while a video plays or tasks are pending, else 250 ms.

### Files to Touch
`src/app.rs`: press filter, mouse handling, dirty tracking in `run_tui`, `drain_events` returns applied count.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] `handle_key` ignores non-Press key events.
- [x] `run_tui` handles mouse wheel events as selection movement; other mouse events ignored.
- [x] `drain_events` reports whether any event was applied.
- [x] `run_tui` skips `terminal.draw` when nothing changed; resize and status-expiry mark dirty.
- [x] Poll timeout 50 ms during video playback or pending tasks, 250 ms otherwise.
- [x] Tests: release key event produces no action; wheel event moves selection; drain_events count.
- [x] Gate: fmt, test, clippy -D warnings, build.

### Acceptance Criteria
**Scenario:** Typing on a kitty-protocol terminal.
**GIVEN** the terminal reports Press and Release events.
**WHEN** the user presses `j` once.
**THEN** the selection moves exactly one item.

**Scenario:** Idle app.
**GIVEN** no pending tasks, no video, no input.
**WHEN** 10 seconds pass.
**THEN** no redraw occurs and CPU stays near zero.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
No blockers. Wheel handling routes through the same after-input hook as keys so scrolling near the bottom still triggers pagination. Idle-CPU acceptance was validated by reasoning over the loop structure and unit tests on the pieces (drain count, advance_video_frame bool); no automated CPU measurement was added.
