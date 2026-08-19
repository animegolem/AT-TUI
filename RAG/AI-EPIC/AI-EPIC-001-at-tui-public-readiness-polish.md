---
node_id: AI-EPIC-001
tags:
  - EPIC
  - AI
  - at-tui
  - public-readiness
  - polish
date_created: 2026-05-22
date_completed: 2026-08-19
kanban_status: completed
AI_IMP_spawned:
  - AI-IMP-001-1
  - AI-IMP-001-2
  - AI-IMP-001-3
---

# AI-EPIC-001-at-tui-public-readiness-polish

## Problem Statement/Feature Scope
AT-TUI is now usable enough that small interaction gaps and visual rough edges are more noticeable than missing core browsing features. The next work should convert the remaining parking-lot notes into trackable, reviewable units while keeping deferred 1.0 mouse and layout ideas out of the current sprint.

## Proposed Solution(s)
Create a focused public-readiness polish epic with two implementation tickets and one research spike. The first ticket addresses compact UI polish and readability issues in timeline rows. The second adds follow/unfollow as the next account-action primitive. The third produces a concrete video-audio implementation brief so future video work starts with a known path instead of open-ended investigation.

## Path(s) Not Taken
Comprehensive mouse interaction, wide responsive side panes, profile follow buttons with mouse affordances, keymap-file support, and full video/audio playback implementation are out of scope for this epic. Those remain later product passes after the current keyboard-first surfaces stabilize.

## Success Metrics
Within this epic, AT-TUI should have a compact statusline without visual divider noise, readable wrapped media/link summaries, clear active engagement styling, a working keyboard follow/unfollow path, and a video-audio brief that can be used to start a follow-up implementation ticket without additional discovery.

## Requirements

### Functional Requirements
- [x] FR-1: The statusline shall render compact colored segments without literal slash separators.
- [x] FR-2: Media alt text and external link descriptions shall wrap within the timeline width.
- [x] FR-3: Media and link summary lines shall use distinct colors from normal post body text.
- [x] FR-4: Active liked posts shall render with a red bold heart, matching active repost state clarity.
- [x] FR-5: Users shall be able to follow or unfollow the selected/profile account with a provisional keyboard action.
- [x] FR-6: Profile views shall expose follow state in the header.
- [x] FR-7: Video-audio work shall produce a brief covering implementation options, risks, and a recommended v1 path.

### Non-Functional Requirements
- Keep the app keyboard-first and terminal-portable.
- Do not require Nerd Font, terminal mouse support, or a browser-based OAuth flow.
- Preserve the current test gate: format check, unit tests, clippy with warnings denied, build, and diff whitespace check.

## Implementation Breakdown
- [x] [[AI-IMP-001-1-cosmetic-polish]]: Compact statusline and timeline readability polish.
- [x] [[AI-IMP-001-2-follow-unfollow-accounts]]: Keyboard follow/unfollow support.
- [x] [[AI-IMP-001-3-video-audio-spike]]: Research artifact for video audio.
