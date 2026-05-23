---
node_id: AI-IMP-001-1
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - ui
  - polish
kanban_status: planned
depends_on: [[AI-EPIC-001-at-tui-public-readiness-polish]]
parent_epic: [[AI-EPIC-001-at-tui-public-readiness-polish]]
confidence_score: 0.88
date_created: 2026-05-22
date_completed:
---

# AI-IMP-001-1-cosmetic-polish

## Compact Statusline And Timeline Readability
The current compact statusline still uses literal slash separators between colored sections. Timeline media/link summaries also read too much like normal body text, and long descriptions can overflow instead of wrapping cleanly. This ticket makes the UI denser and easier to scan without adding font or terminal dependencies.

### Out of Scope
Powerline/Nerd Font glyphs, a theme system, mouse affordances, settings UI changes, and keymap-file support are not part of this ticket.

### Design/Approach
Keep the statusline one row. Remove separator spans and let colored segments sit edge-to-edge with internal padding. Preserve right-pinned selected/total rendering. Rework media and external-card summary rendering so descriptions wrap through the existing line-wrapping helpers, then apply distinct colors for media and links. Active engagement styling should remain glyph-based and terminal-stable: liked uses red bold `♥`; reposted remains green bold `⟳`.

### Files to Touch
`src/ui.rs`: statusline segment rendering, media/link summary wrapping, color styling, and tests.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [ ] Remove literal slash separator spans from statusline construction.
- [ ] Ensure colored statusline segments connect edge-to-edge while retaining readable padding.
- [ ] Keep `status_right_line` separately right-aligned and unchanged in width behavior.
- [ ] Wrap image alt text within available row width.
- [ ] Wrap video alt text within available row width.
- [ ] Wrap external link title/description summaries within available row width.
- [ ] Apply distinct media/link colors so summaries stand out from normal post text.
- [ ] Style active likes with red + bold `♥`.
- [ ] Preserve active repost styling as green + bold `⟳`.
- [ ] Update or add unit tests for status segments, media/link wrapping, and active engagement colors.

### Acceptance Criteria
**Scenario:** A timeline row includes a long image alt text.  
**GIVEN** the terminal is narrow.  
**WHEN** the row is rendered.  
**THEN** the image summary wraps within the post width.  
**AND** the summary is visually distinct from body text.

**Scenario:** A post is liked and reposted by the active account.  
**GIVEN** viewer like and repost state are present.  
**WHEN** engagement metadata is rendered.  
**THEN** the heart is red and bold.  
**AND** the repost marker is green and bold.

**Scenario:** The statusline is rendered.  
**GIVEN** account, location, and pending segments are present.  
**WHEN** the one-row statusline is drawn.  
**THEN** no literal slash separator appears between colored status segments.  
**AND** the selected/total counter remains pinned at the right.

### Issues Encountered
None yet.
