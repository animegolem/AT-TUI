---
node_id: AI-IMP-002-4
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - keymap
  - input
kanban_status: completed
depends_on: [[AI-IMP-002-2]]
parent_epic: [[AI-EPIC-002-at-tui-stabilization-daily-driver]]
confidence_score: 0.87
date_created: 2026-07-06
date_completed: 2026-07-06
---

# AI-IMP-002-4-keymap-rework-page-scroll

## Summary of Issue #1
The action keys were assigned ad hoc: `q` (quit) sits next to `Q` (quote), like is on shifted `F`, reply on `c`, reload on `r`, follow on `w`, and there is no page scrolling at all. Done state: the epic's agreed keymap is live — lowercase acts on the post, shift acts on the view/person, `q` layers back-then-quit — with `Ctrl-d`/`Ctrl-u`/`PgDn`/`PgUp` scrolling, and the help overlay and README updated in the same change.

### Out of Scope
Keymap config files, chord bindings, remapping the sacred navigation keys (`h/j/k/l`, arrows, Enter, Esc, `g/G`, `[`/`]`, Space, `/`, `n`, `?`).

### Design/Approach
Per the epic table: `q` back/close (quits only at root timeline with no overlay), `f` like, `F` follow, `b` repost, `r` reply, `R` reload, `Q` quote, `o` open link(s), `e` view embedded quote, `u` load pending, `p` compose (unchanged), `d` reserved (delete lands in 002-6); `w`, `U`, `c`-as-reply retired. Page motion: add `ViewState::move_by(isize)` stepping the selection N items (half page = 5 items, full = 10 — item-based since rows have variable height); `Ctrl-d`/`Ctrl-u` half, `PgDn`/`PgUp` full. All changes live in `normal_action_for_key`, so the Action enum barely moves; update `normal_key_help_lines`, README Keys section, and the action-registry test.

### Files to Touch
`src/app.rs`: key table, `q` layering, page-scroll actions, tests.
`src/navigation.rs`: `move_by`.
`src/ui.rs`: help text.
`README.md`: Keys section.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] `normal_action_for_key` implements the agreed table; retired keys unmapped.
- [x] `q` pops views/overlays and quits only at root with no overlay.
- [x] `navigation.rs`: `move_by` clamped at both ends; unit test.
- [x] `Ctrl-d`/`Ctrl-u` half-page, `PgDn`/`PgUp` full-page in all list views.
- [x] Help overlay lines match the new map.
- [x] README Keys and Menu sections match the new map.
- [x] Tests updated: action registry, q layering, page motion.
- [x] Gate: fmt, test, clippy -D warnings, build.

### Acceptance Criteria
**Scenario:** Backing out with q.
**GIVEN** a thread view pushed above the timeline.
**WHEN** the user presses `q` twice.
**THEN** the first pop returns to the timeline and the second quits the app.

**Scenario:** Liking and following.
**GIVEN** a selected post by another author.
**WHEN** the user presses `f` then `F`.
**THEN** the post is liked and the author followed, in that order.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
No blockers. `q` inside overlays already closed them via the overlay handlers, so BackOrQuit only needed the normal-mode layering. `d` is intentionally left unmapped until AI-IMP-002-6 claims it for delete. Page motion is item-based (5/10) rather than pixel-based because rows have variable height; feels right in practice but worth revisiting if items get much taller with inline media.
