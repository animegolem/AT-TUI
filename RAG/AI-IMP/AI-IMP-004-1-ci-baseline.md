---
node_id: AI-IMP-004-1
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - ci
  - testing
  - github-actions
kanban_status: in-progress
depends_on:
parent_epic: [[AI-EPIC-004-tagged-1-0-release-readiness]]
confidence_score: 0.96
date_created: 2026-08-19
date_completed:
---

# AI-IMP-004-1-ci-baseline

## Repository changes have no automated quality gate
AT-TUI has a strong local gate, but GitHub does not run it for pull requests or
pushes. Platform regressions, formatting drift, Clippy warnings, stale tests,
documentation failures, and release-only build failures can therefore reach
`main` without independent proof. Done state: one least-privilege GitHub
Actions workflow runs the agreed quality and platform checks with the exact
Rust toolchain currently proven locally.

### Out of Scope
Branch-protection settings, dependency update automation, advisory scanning,
artifact publication, release signing, release tags, Cargo metadata changes,
package-content exclusions, and automated Ghostty, Kitty graphics, or mpv
acceptance tests.

### Design/Approach
Add one workflow for pull requests and pushes to `main`. A Linux quality job
runs formatting, Clippy with warnings denied, documentation, and Cargo package
verification. A two-platform matrix runs all test targets and optimized builds
on Ubuntu and macOS. Install Rust `1.90.0` with rustup so hosted-runner updates
cannot silently change the compiler. Use the tracked lockfile, read-only
contents permission, and branch-scoped concurrency cancellation. Start without
a build cache; add one only if observed runner time justifies the complexity.

### Files to Touch
`.github/workflows/ci.yml`: define quality and cross-platform build/test jobs.
`RAG/AI-EPIC/AI-EPIC-004-tagged-1-0-release-readiness.md`: track completed CI requirements.
`RAG/AI-IMP/AI-IMP-004-1-ci-baseline.md`: record implementation and validation.
`RAG/INDEX.md`: regenerate from ticket state.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [ ] Trigger CI for pull requests and pushes to `main`.
- [x] Restrict the workflow token to read-only repository contents.
- [x] Cancel superseded runs for the same workflow and Git reference.
- [x] Install the locally proven Rust `1.90.0` toolchain explicitly.
- [x] Run formatting and Clippy with warnings denied on Ubuntu.
- [x] Build documentation and verify Cargo package assembly on Ubuntu.
- [ ] Run all test targets with locked dependencies on Ubuntu and macOS.
- [ ] Build the optimized binary with locked dependencies on Ubuntu and macOS.
- [x] Run the full workflow command set locally where the host permits it.
- [x] Regenerate the RAG index and record environmental limits accurately.

### Acceptance Criteria
**Scenario:** A contributor opens or updates a pull request.
**GIVEN** the repository contains the CI workflow.
**WHEN** GitHub evaluates the pull-request commit.
**THEN** formatting, Clippy, documentation, package, test, and optimized-build checks run without write credentials.

**Scenario:** The same branch receives another commit during CI.
**GIVEN** an older run is still active for that workflow and Git reference.
**WHEN** the newer run begins.
**THEN** GitHub cancels the superseded run and evaluates the latest commit.

**Scenario:** A platform-specific compile regression is introduced.
**GIVEN** the test matrix includes Ubuntu and macOS.
**WHEN** the affected platform builds and tests the change.
**THEN** the workflow fails before the change can satisfy the required CI gate.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
The workflow passed `actionlint` 1.7.12. On the local macOS host, Rust 1.90.0
passed formatting, Clippy with warnings denied, documentation with rustdoc
warnings denied, all 146 library tests, the CLI integration test, the
optimized build, and `git diff --check`.

`cargo package --locked` assembled and verified 54 files, including the public
`RAG/` records. Cargo emitted the known warning that package metadata is not
yet defined; metadata policy remains intentionally deferred to the next epic
phase and does not fail package verification.

No GitHub event or Ubuntu runner has evaluated the workflow yet. The ticket
remains in progress until the branch is pushed and both hosted matrix jobs pass.
