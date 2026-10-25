---
node_id: AI-IMP-003-2
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - ui
kanban_status: planned
depends_on:
  - AI-IMP-003-1
parent_epic: [[AI-EPIC-003-at-tui-release-review]]
confidence_score: 0.75
date_created: 2026-10-25
date_completed:
---

# AI-IMP-003-2-ui-refresh-proposals

## Summary of Issue #1
The UI is functional but hard to scan: the selection marks only the header line, every post carries `↩ 0 ⟳ 0 ♡ 0 ❞ 0`, alt text and card descriptions run to four lines, threads show depth only by indentation, notifications list raw `at://` URIs one like per row, and the help screen doubles as an account switcher where Tab switches instantly. Done state: the owner's approved subset of the 20 proposals in `~/review/at-tui-review.html` is merged, and skipped items are removed from the prototype.

### Out of Scope
Anything the owner skips; inline image thumbnails; theming/config files.

### Design/Approach
Prototype on `proposal/ui-refresh` (unmerged) covering proposals 1–15: full-height gutter selection, header hierarchy with right-aligned time, per-author colours (FNV of DID), quiet engagement with compact counts, inline mention/tag/link colouring, one line per attachment, muted quote frames, thread tree guides with a flat ancestor chain and a ◆ anchor, grouped notifications with subject text (`getPosts`), profile header, calmer statusline with breadcrumb and red errors, grouped help card, Tab switcher with Enter, composer upgrades (^W/^U, ^E to $VISUAL/$EDITOR), connecting splash. Proposals 16–20 (Esc never opens, quit guard, viewport paging, content labels, composer cursor) are described only. Renders come from a scratch harness that draws fixture JSON through `ui::render` into a TestBackend; it is not committed.

### Files to Touch
`src/ui.rs`, `src/app.rs`, `src/model.rs`, `src/api.rs` (`get_posts`), README key list.

### Implementation Checklist
- [x] Prototype 1–15 with tests (138 passing, gate green) on `proposal/ui-refresh`.
- [ ] Record the owner's decision per proposal.
- [ ] Remove skipped items; implement approved 16–20.
- [ ] Update README keys (Tab, ^E, ^W/^U) and help card together.
- [ ] Rebase onto `review/release-fixes`, gate, merge.

### Acceptance Criteria
**GIVEN** the owner's decision list **WHEN** the branch merges **THEN** every approved item is present, every skipped item is absent, and the README, help card and keymap agree.

### Issues Encountered
`^E` was implemented in the prototype so its hint is truthful; a fake-editor run verified the suspend/resume round trip. Live terminal glyph fallback for `⟳ ❞ ◆` varies by font; all glyphs are non-Nerd-Font Unicode.
