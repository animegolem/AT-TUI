# AT-TUI Handoff: Idle Session Refresh

Date: 2026-08-16

> **Historical handoff:** Later work verified the real checkout, added shared
> session state and single-flight refresh, handled typed HTTP 400
> `ExpiredToken` responses, and scoped background task results. The 24-hour,
> two-expiry live validation remains open in AI-IMP-003-7. Use the current RAG
> tickets and source code for implementation status.

## User-Visible Bug

After the application sits idle for long enough, feed and notification updates stop. Closing and reopening the application restores updates. The leading hypothesis is stale in-memory session state after Bluesky rotates refresh tokens.

## Important Verification Caveat

The current Codex task could not launch any shell process. Both `/bin/zsh` and `/bin/bash` failed before command execution with:

```text
Failed to create unified exec process: No such file or directory (os error 2)
```

Consequently, the current worktree, recent commits, and implementation details below were not freshly verified in this task. Start by inspecting the live repository rather than treating this note as authoritative.

## Last Known Repository State

- Follow/unfollow support had been implemented and validated, but may still be uncommitted.
- The related RAG implementation tickets and index had been updated.
- Earlier validation reportedly passed: formatting, tests, clippy with warnings denied, build, and `git diff --check`.
- Video currently appeared to buffer all decoded frames before returning them to the UI; progressive frame events were the proposed next video spike.

## Likely Session Failure Mode

The app appears to clone `BskyClient` into background tasks. A clone that receives a `401` may refresh successfully and persist rotated access and refresh JWTs, while the long-lived `App` client retains the old in-memory refresh token. Bluesky refresh tokens rotate, so a later task cloned from the stale app client can no longer refresh. Restarting works because the new process reloads the recently persisted session.

This is a hypothesis until confirmed against the current `BskyClient`, task, and event code.

## First Inspection Pass

Run:

```sh
git status --short --ignored
git log --oneline -10
git diff --stat
rg -n "refresh_session|client\.clone|BskyClient|Session|AppEvent|WriteCompleted|FeedLoaded|NotificationCountLoaded" src/*.rs
```

Then inspect session persistence and every background task result that can trigger refresh-on-401.

## Recommended Fix Shape

1. Make task completion carry the task client's final session state back to the UI event loop whenever it changed.
2. Apply that session only when its DID/account identity still matches the active account and request context.
3. Update the long-lived app client and persist the account-scoped session from one explicit reconciliation path.
4. Do not allow a stale task from a prior account to overwrite the active account's credentials.
5. Avoid solving this only by rereading the session file before every request; explicit task results make ownership and stale-event handling testable.

An equivalent shared session coordinator may be reasonable if the current code already has one, but it must serialize refresh-token rotation and expose updated credentials to future client clones.

## Required Regression Tests

- A cloned client refreshes after `401`, rotates tokens, and persists the new session.
- The app applies the refreshed session returned by a background task.
- A second background request uses the rotated refresh token without an application restart.
- A stale result for another account cannot overwrite the active account session.
- Concurrent `401` responses do not independently race the same refresh token if concurrent refresh is possible.

## Validation

After implementation, run:

```sh
cargo fmt -- --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
git diff --check
```

Also perform a live idle test against Bluesky long enough to cross access-token expiry, because mocked `401` tests alone do not prove refresh-token rotation behavior in the running application.
