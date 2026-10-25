---
node_id: 2026-10-25-LOG-AI-release-review-fixes-and-ui-proposals
tags:
  - AI-log
  - development-summary
  - at-tui
  - release
closed_tickets:
  - AI-IMP-003-1
created_date: 2026-10-25
related_files:
  - src/api.rs
  - src/app.rs
  - src/navigation.rs
  - src/model.rs
  - src/ui.rs
  - src/config.rs
  - src/media.rs
  - src/main.rs
confidence_score: 0.88
---

# 2026-10-25-LOG-AI-release-review-fixes-and-ui-proposals

## Work Completed
Full read of `src/`, a live run against the owner's saved account (read-only; no posts, likes or follows; notifications never opened, so `updateSeen` was never called), and empirical probes. Headline: Bluesky returns `400 ExpiredToken` for an expired access token, so EPIC-002's refresh never fired and every session went offline at the 2-hour mark. Fixed fifteen defects on `review/release-fixes` (AI-IMP-003-1) and prototyped the UI refresh on `proposal/ui-refresh` (AI-IMP-003-2), presented as real before/after renders in `~/review/at-tui-review.html` for per-item approval.

## Session Commits
- 24e318b Refresh on 400 ExpiredToken and add HTTP timeouts
- bb77fac Track pagination per view; keep errors visible
- 7bfd7fa Input safety: bracketed paste, global Ctrl-C, web-only links, kept drafts
- 5517cc3 Feed correctness: real names, official reply filter, failed-switch revert
- 56028f6 Atomic account writes, bounded disk cache, forgiving login handle
- (proposal branch) PROPOSAL (unmerged): UI refresh prototype for owner review

## Issues Encountered
A shared cargo target dir served a stale binary to the first after-fix probe (filed to the house ledger). The proposal-branch commit predates this log commit and needs a rebase before merge.

## Tests Added
114 → 131 on the fixes branch (expiry classification with the live-captured body, refresh-token adoption, per-view pagination after navigating away, error row visibility via the real renderer, paste routing, Ctrl-C from overlays, web-only links, draft retention, failed feed switch, reply-filter semantics/default, feed display names, minute-keyed cache, atomic config mode, cache pruning). 138 on the proposal branch.

## Next Steps
- Owner records decisions on proposals 1–20; trim the prototype accordingly and merge.
- Proposal 19 (content labels) should precede any public release.
- Add an HTTP-mock integration test for the 400/401 refresh paths and concurrent refresh.
- Backlog: scrolling inside over-tall posts, offline-tolerant launch, PDS resolution for non-bsky.social accounts, viewer state on posts opened via `e`.
