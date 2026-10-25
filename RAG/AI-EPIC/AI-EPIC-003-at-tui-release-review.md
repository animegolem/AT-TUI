---
node_id: AI-EPIC-003
tags:
  - EPIC
  - AI
  - at-tui
  - release
  - review
date_created: 2026-10-25
date_completed:
kanban_status: in-progress
AI_IMP_spawned:
  - AI-IMP-003-1
  - AI-IMP-003-2
---

# AI-EPIC-003-at-tui-release-review

## Problem Statement/Feature Scope
A release-readiness review of AT-TUI found that the daily-driver promise of EPIC-002 did not hold: every session went offline two hours after launch, because Bluesky answers an expired access token with `400 ExpiredToken` and the client only refreshed on 401. Around it sat dead infinite scroll after navigating mid-load, input hazards (paste fired like/repost/follow; any URI scheme was handed to the OS), lost drafts, wrong feed names, an inert reply filter, and a UI that is functional but hard to scan (raw `at://` URIs in notifications, four-line alt text, no thread structure).

## Proposed Solution(s)
Two tracks. **Fixes** (IMP-003-1) land on `review/release-fixes` with tests and live verification, no keymap or visual changes beyond making errors visible. **UI refresh** (IMP-003-2) is prototyped on the unmerged `proposal/ui-refresh` branch and presented to the owner as real before/after renders of identical fixture data (`~/review/at-tui-review.html`); each of 20 numbered proposals is approved, skipped or discussed individually before anything merges.

## Path(s) Not Taken
Changing ratified EPIC-002 keymap behaviour (Esc at root, quit semantics, page motion) as part of the fixes; those are proposals 16–18. OAuth, DMs, image upload and list feeds remain out of scope.

## Success Metrics
- A request made with a stale access token refreshes and succeeds (verified live against bsky.social, 2026-10-25).
- Opening a thread while a timeline page loads no longer stops pagination (verified live: 42 → 82 → 113).
- A paste in the timeline fires no actions (verified live with bracketed paste).
- Gate green: fmt, tests (114 → 131), clippy `-D warnings`, build.
- Owner records a decision for each UI proposal.

## Requirements

### Functional Requirements
- [x] FR-1: Refresh on `400 ExpiredToken` as well as 401; a rejected refresh token reports "run at-tui login".
- [x] FR-2: All HTTP requests time out.
- [x] FR-3: Pagination results reach the view that requested them.
- [x] FR-4: View errors are visible; failure statuses persist 8 s.
- [x] FR-5: Pastes only reach the composer or search; Ctrl-C quits everywhere.
- [x] FR-6: Only http(s) links are opened.
- [x] FR-7: Drafts survive Esc and failed sends.
- [x] FR-8: Saved feeds show their display names; the home reply filter follows the official default and checks the right people; a failed feed switch reverts.
- [x] FR-9: Relative timestamps stay current; account config writes are atomic; disk cache is bounded.
- [ ] FR-10: Owner-approved UI proposals merged (1–15 merged 2026-10-25; 16–20 pending a decision).

### Non-Functional Requirements
- No new dependencies. Every behavioural fix lands with a test; the three headline fixes are also verified against the live service.

## Implementation Breakdown
- [x] [[AI-IMP-003-1-release-review-fixes]]: FR-1..9.
- [ ] [[AI-IMP-003-2-ui-refresh-proposals]]: FR-10, gated on owner approval.
