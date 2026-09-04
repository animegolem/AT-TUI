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
