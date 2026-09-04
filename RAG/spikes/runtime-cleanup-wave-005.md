# Runtime cleanup wave 005

Lead: Codex/root. Baseline: 6be2bc3. Integration: codex/runtime-cleanup-review;
merge destination: the existing codex/ci-baseline branch. Date: 2026-09-04.
Governing scope: [[AI-EPIC-005-runtime-correctness-and-efficiency]], section 1.

## Assignment and ownership

The implementation lead receives AI-IMP-005-1 through AI-IMP-005-6. Ticket IDs
are reserved. Use an isolated clone; do not edit the owner's checkout. The
implementation lead may delegate navigation (005-1, 005-2) and media (005-3,
005-4) to at most two subagents in separate clones. Integrate those atomic
commits before 005-5 and 005-6. Reconcile shared app.rs edits explicitly.

Round 1 is source-grounded review only: verify diagnoses, propose ownership
and cache-maintenance semantics, identify interactions, list tests and repair
scope. Send the report to the root lead and wait for its implementation ruling.
No implementation begins before the ruling. The lead owns design and tickets.

Round 2 produces one atomic commit per ticket in dependency order. Perform
self-review, then report commits, touched files, tests/counts, untracked material,
removed symbols, performance evidence, and remaining concerns. The root lead
reviews load-bearing boundaries and independently reproduces the full gate.
All agents are forbidden from pushing, merging the owner's branches, or
removing branches/worktrees/refs. Use local files for durable reports.

## Fences

Only ticket-listed source files and their focused tests may change. Ask the
root lead if a file/interface expansion is necessary. Do not change Cargo
versions, auth/config persistence behavior, key bindings, public README copy,
CI/release setup, AGENTS.md, existing tickets, or the root-owned RAG record.
Do not use real Bluesky accounts or send network writes for testing. Synthetic
and mock data must supply race and terminal-render coverage. Preserve narrow
layout, reading position, existing background behavior, and mpv handoff.

## Validation

At each ticket: targeted meaningful regressions plus formatting and a clean
whitespace diff. At submission and again at integration:

```sh
cargo fmt --all -- --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps
cargo build --locked --release
git diff --check
```

The root lead also runs `cargo package --locked` after reviewed commits and a
synthetic terminal/render smoke. Observe exact pass/fail counts, not a filtered
pipeline exit. Test-only compatibility helpers are not automatic dead code.
The root regenerates the RAG index after ticket changes and records final
acceptance; agent reports alone do not close tickets.

## Review record

Planning: created six source-backed tickets and regenerated the index. Prior
review reproductions, outside the checkout, confirmed the three interaction
failures and lifetime cache-pin size-target violation. Existing baseline passed
146 unit tests, one CLI integration test, formatting, and Clippy.

## Round 1 verdict — accepted for implementation

2026-09-04. Reviewed the navigation, media, and cleanup/layout reports against
current source. Implement 005-1 through 005-6 within the ticket fences.

- Navigation: use nonreused view-instance identities, foreground intent and
  account checks, distinct surviving-root targeting, and one Back/pop path.
  Do not use kind, actor, or URI alone as identity. Preserve legitimate root
  loads beneath pushed views. Pagination is owned per view/request and old
  completions cannot release a newer page or full-load token. Background page
  errors belong to the background view, not the foreground statusline.
- Media: allow scheduler admission for already-loading demand; promote normal
  in-flight speculation as well as reviving cancelled work. Refresh generation
  for renewed demand without duplicating physical jobs or resetting attempts.
- Cache: remove lifetime final-file pins. Await atomic publication on the
  blocking pool within existing bounded media jobs, then request asynchronous
  coalesced maintenance. Do not create an unbounded detached write queue. Owned
  bytes permit eviction without blocking decode. Maintenance requested during
  a pass must produce a final pass. Temporary publication files must also have
  a cleanup story after failure/restart; do not ignore orphan staging files
  forever. Background startup maintenance must not introduce a hidden panic
  requirement on a synchronous constructor without a Tokio runtime.
- Cleanup: the verified symbols are candidates, not a license to remove all
  public/test helpers. Route the disk-hit test through run_limited and preserve
  README keymap generation, preview_images, and source label methods with users.
- Layout: preserve ordinary cached-timestamp behavior; removing incidental
  timestamp refresh on append is accepted without adding a new timer. Full
  notification-mutation invalidation is allowed. Represent or explicitly test
  selected/unselected height equivalence before relying on prefix offsets.
  Record actual work counts and buffer equivalence rather than a speed claim.

Independent baseline validation: the root's localhost mock-server PTY fixture
reproduced the timeline remaining `Following [loading]` after pagination finished
under a pushed thread. The fixture uses an isolated account file and synthetic
responses only. Formatting/structure checks passed for the epic and six tickets.
