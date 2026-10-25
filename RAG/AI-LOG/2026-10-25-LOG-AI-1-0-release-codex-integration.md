---
node_id: 2026-10-25-LOG-AI-1-0-release-codex-integration
tags:
  - AI-log
  - development-summary
  - at-tui
  - release
closed_tickets:
  - AI-IMP-006-2
created_date: 2026-10-25
related_files:
  - src/app.rs
  - src/media.rs
  - src/video_player.rs
  - src/config.rs
  - tests/cli_accounts.rs
  - .github/workflows/ci.yml
  - README.md
  - Cargo.toml
confidence_score: 0.88
---

# 2026-10-25-LOG-AI-1-0-release-codex-integration

## Work Completed
The owner asked for the "other working branch" to be checked, merged, closed, and 1.0 released. `codex/ci-baseline` turned out to be a full parallel line: 28 commits from 2026-08-17 to 10-25, never merged, and it had already fixed the ExpiredToken bug in August. On the owner's direction main stayed the base, and that line's features were ported by hand in PR #3:
- CI.
- mpv video, replacing ffmpeg frames, with a windowed fallback and a shared terminal hand-off.
- The CLI account test and the `AT_TUI_ACCOUNTS_FILE` override.
- Cancelling in-flight loads when going back.

The branch was then recorded as merged with `-s ours`. The PR also set version 1.0.0, rewrote the README, re-recorded the demo, and moved agent records to `working`. Tagged `v1.0.0`, with a GitHub Release.

## Issues Encountered
- Epic numbering collided: both seats minted 003/004. My records were renumbered here: 006 release review, 007 Mastodon, 008 carry-over.
- Live tmux tests looked broken until I found the first keystroke swallowed after the image-protocol query. The August binary does the same, so it isn't a regression; logged in EPIC-008.
- The fail-before check for the back-cancel test was invalid: stashing removed the new tests along with the fix, so it proved nothing.

## Tests Added
CLI round-trip (ported), mpv windowed-fallback args, back-out-before-thread-lands, q-cancels-pending, session image line, successful reply status. Total: 144 unit + 1 integration.

## Next Steps
See [[AI-EPIC-008-1-1-carryover]]. Proposal 19 (content labels) comes before any wider announcement.
