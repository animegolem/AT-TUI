---
node_id: 2026-09-04-LOG-AI-runtime-cleanup-epic-005-complete
tags:
  - AI-log
  - development-summary
  - at-tui
  - reliability
  - performance
closed_tickets:
  - AI-IMP-005-1
  - AI-IMP-005-2
  - AI-IMP-005-3
  - AI-IMP-005-4
  - AI-IMP-005-5
  - AI-IMP-005-6
created_date: 2026-09-04
related_files:
  - src/api.rs
  - src/app.rs
  - src/media.rs
  - src/media_scheduler.rs
  - src/model.rs
  - src/navigation.rs
  - src/ui.rs
  - src/video_player.rs
confidence_score: 0.95
---

# 2026-09-04-LOG-AI-runtime-cleanup-epic-005-complete

## Session Handoff — Development Paused

On 2026-09-04, after accepting the completed work, the owner said there is no active development on the app at present and requested a session log for future reference. This is a resting point, with no further implementation assigned by this sitting. Resume development when the owner asks.

The six reviewed ticket commits and their closure record were fast-forward merged into the original local codex/ci-baseline branch at dd6abab. The checkout was clean when this handoff began; that merge contains exactly the validated source from 4314ef1. Nothing from this sitting was pushed or released. This handoff is a documentation-only follow-up.

For a future session, start with this log, [[AI-EPIC-005-runtime-correctness-and-efficiency]], RAG/spikes/runtime-cleanup-wave-005.md, and a fresh Git status. Preserve the reading viewport, explicit view/request ownership, background I/O, bounded media scheduling, and app-password auth. Check live branch, ticket, and environment state before relying on this snapshot. Temporary clone/test artifacts may expire; the committed source regressions and this evidence record are the durable handoff.

The older open records are retained for context, not a new assignment: epic 003 still has live idle/terminal acceptance and planned diagnostics work, while epic 004 retains hosted CI/release readiness. Local tests and the synthetic terminal smoke do not close those gates. No code or test changes were needed for this final logging step.

## Work Completed

Completed [[AI-EPIC-005-runtime-correctness-and-efficiency]] and its six tickets from baseline 6be2bc3. One implementation lead coordinated navigation and media work in isolated clones, then integrated cleanup and rendering changes. The root reviewer owned the contract, implementation rulings, source review, independent validation, and local integration. The atomic source series ends at 4314ef1; documentation closure dd6abab followed it on codex/runtime-cleanup-review and was fast-forward merged into the original local codex/ci-baseline branch.

- View requests now identify their origin instance. Back invalidates foreground navigation intent, late results cannot overwrite another view, and valid root loads still complete beneath pushed views.
- Pagination belongs to a view/request pair. Retained background views receive their results and release their own loading state; older results cannot release newer work. Reload suppresses pagination using an outgoing cursor.
- Visible image demand can promote or revive existing scheduler work while preserving download deduplication, physical worker limits, and retry accounting.
- Disk cache uses atomic publication and coalesced background maintenance. Current-session files participate in the existing byte/age policy, and image completion does not wait for directory scans.
- Twelve obsolete helpers and the duplicate image loader were removed. The cache-hit test now exercises production loading, and publication/decode no longer make two full downloaded-buffer copies.
- Appended rows reuse cached layouts. Prefix offsets locate the viewport, and redraws follow visible state changes, spinner ticks, status expiry, input, and resize.

## Accepted Source Commits

| Ticket | Root commit | Change |
| --- | --- | --- |
| 005-1 | 1e31c16 | Bind view completions to their origin instance |
| 005-2 | 599f170 | Retain pagination ownership across navigation |
| 005-3 | 267206e | Revive and promote visible image requests |
| 005-4 | eeb6098 | Bound disk cache with coalesced background maintenance |
| 005-5 | 13a872a | Remove obsolete helpers and use production image loader |
| 005-6 | 4314ef1 | Reuse appended layouts and redraw on visible changes |

Source at 4314ef1 matches the implementation lead's final 3d74093. Planning/review records are c5f561a, 16f5444, and 1b1be42. The source series respects ticket dependencies; independent navigation/media commits are interleaved. No dependency, auth/config persistence, key binding, README, or CI/release change was needed.

## Review Rulings and Issues Resolved

- Kind/URI equality is insufficient request ownership; view instances receive nonreused identities. Foreground, surviving-root, and retained-background completion rules are separate. Review covered Back aliases, replacement, account scope, and old/new token interactions.
- Profile and notification Reload exposed an additional race: after_input could queue a page from the outgoing cursor. The implementation adds a matching-request reload token and tests that event-loop follow-up.
- Cache bounds apply after successful quiescent maintenance. Active in-process staging is temporarily protected; final files have no session exemption. Unknown/inactive staging has no grace period. Another process can lose a best-effort cache publication, but owned image bytes still decode. Failed I/O is nonfatal, and a later publication can retry maintenance.
- Publication remains awaited on the blocking pool inside bounded media jobs; the implementation does not create a detached unbounded write queue. Maintenance generation and idle transitions share a lock so work requested during a scan cannot lose the final pass. Startup also works without a Tokio runtime.
- Layout review checked selected/unselected height differences and full invalidation for mutation, replacement, removal, prepend, and resize, including notification mutation. Existing timestamp-cache behavior remains; removing incidental timestamp refresh on append was accepted without adding a timer.

No unresolved ticket-scoped blocker or missing acceptance test remains. Older live idle/terminal and hosted CI/release gates were not closed by this work.

## Removed Symbols

Caller checks covered source, tests, examples, and documentation. Removed BskyClient::login; NavigationStack::breadcrumb; PreviewMedia::source_label; HomeFeedPrefs::status_label; MpvPlayer::executable; MediaCache::{should_load, mark_loading, load_job, should_load_url}; and ImageLoadJob::{url, run, load}. This is 12 helpers. Useful keymap/README verification helpers, preview_images, and source label methods with callers remain. The disk-hit/no-network regression now invokes run_limited with scheduler execution limits.

## Independent Validation

Root ran the following at source 4314ef1 with Rust/Cargo 1.90.0; every command exited zero:

```sh
cargo fmt --all -- --check
cargo test --locked --all-targets --quiet
cargo clippy --locked --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps
cargo build --locked --release
cargo package --locked
git diff --check
```

Tests: 182 library tests plus one CLI integration test passed; zero failures, ignored tests, or filtered tests. The baseline had 146 library tests plus one CLI test, giving 36 net additional library regressions. Package verification included rebuilding the packaged crate. Cargo emitted its existing optional manifest-metadata warning; package creation and verification succeeded, and Cargo.lock stayed unchanged.

Regression coverage includes late view successes/errors after all Back aliases; same-kind replacement; retained root loads; concurrent page completion orders; popped/replaced/newer page tokens; reload follow-up; integrated App/cache/scheduler promotion and revival; obsolete media and retry limits; disk budget/age/oversize/corruption/staging/failure/startup behavior; and full fresh-render equivalence across viewport changes.

### Deterministic Efficiency Evidence

| Scenario | Observed result |
| --- | --- |
| Append 50 posts to 2,000 cached posts at the same width | 50 new row pairs built instead of 2,050; selection changes build zero |
| 20 unchanged pending-state checks between spinner ticks | Zero redraw requests; state transitions, ticks, and expiry still redraw |
| 12 publications totaling 72 bytes with maintenance paused | All publications complete; two coalesced passes leave 6 bytes within an 8-byte budget |
| Publication after the cache worker becomes idle | A third maintenance pass runs |
| Production image loading while maintenance is blocked | Image result completes independently of the scan |

The row result is a 97.56% reduction in row-pair builds for that append scenario, not an elapsed-time or end-user speed claim. Render tests compare styled lines and complete Ratatui buffers against an independent fresh-layout/linear-viewport oracle through append, selection, mutation, 12/80-column resize, profile/error/notification/empty views, prepend, replacement, removal, and differing selected row heights.

### Release-Binary Terminal Smoke

Root used a Python standard-library localhost HTTP fixture and PTY against target/release/at-tui with --no-images, an isolated temporary AT_TUI_ACCOUNTS_FILE, and synthetic session/feed responses. The same fixture reproduced the baseline timeline stuck at Following [loading] after its page completed beneath a thread. The final source passed:

1. Startup at 80 columns by 28 rows.
2. Hold a timeline page, open a thread, release the page, return, and load another page successfully.
3. Hold a thread reload, return to the timeline, release the reload, and preserve the timeline.
4. Open a profile, resize to 42 columns by 18 rows, scroll down/up, and restore its leading profile text.
5. Observe zero terminal output bytes during the sampled idle interval after status expiry.
6. Exit cleanly with q.

The final run passed with 11 fixture requests. No saved real account was read or modified, and no Bluesky write was sent. This synthetic smoke does not stand in for the older live Ghostty/media or 24-hour/two-expiry acceptance.

Temporary raw evidence is under /var/folders/xf/rq944nws6psgkhb73fmdftgm0000gn/T/at-tui-wave005-g4ty3f_l: integration-gate.log/json, baseline-smoke, final-smoke, and smoke_runtime.py. The smoke harness SHA-256 is 125ce231cb3a6c3010cef662be87262a419b3ea1293fed4d1024f489f590eb86. Durable regression tests are in the accepted source commits; temporary artifacts and implementation clones were retained for recovery.

## Remaining Separate Work

AI-EPIC-003 live idle/terminal acceptance and AI-EPIC-004 hosted CI/release readiness retain their existing status. This sitting completes local implementation, review, validation, and merge of epic 005; it does not publish a release or push branches.
