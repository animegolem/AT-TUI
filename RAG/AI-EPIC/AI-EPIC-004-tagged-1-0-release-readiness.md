---
node_id: AI-EPIC-004
tags:
  - EPIC
  - AI
  - at-tui
  - release
  - testing
  - ci
date_created: 2026-08-19
date_completed:
kanban_status: in-progress
AI_IMP_spawned:
  - AI-IMP-004-1
---

# AI-EPIC-004-tagged-1-0-release-readiness

## Problem Statement/Feature Scope
AT-TUI is approaching a stable public release, but its quality gates run only
on a developer workstation. The repository cannot yet prove that every change
builds and passes tests on its primary macOS environment and a portable Linux
environment. Package policy, supported-environment claims, hands-on terminal
checks, and the release procedure also need explicit decisions before a
`1.0.0` tag can represent a tested support contract.

## Proposed Solution(s)
Prepare the release in three independently reviewable phases.

**Phase 1 — continuous integration.** Run formatting, Clippy, documentation,
package verification, tests, and optimized builds on GitHub Actions. Pin the
currently proven Rust toolchain, use locked dependencies, restrict workflow
permissions, and test on Ubuntu and macOS.

**Phase 2 — package and distribution contract.** Add truthful Cargo metadata,
choose the supported installation and distribution paths, define supported
platforms and terminals, and decide which repository files ship in a source
package. Public development records such as `RAG/` remain in scope unless the
owner later chooses a narrower package.

**Phase 3 — release acceptance and tag.** Complete automated and hands-on
acceptance checks, record the results, update the version and public release
notes, and create the first `1.0.0` tag only after the release commit passes
all required gates.

## Path(s) Not Taken
This epic does not tag, publish, or distribute the current beta immediately.
It does not hide public planning records, add credentials to pull-request
workflows, or attempt to automate terminal graphics and audio checks before a
reliable runner exists. It does not add unrelated product features.

## Success Metrics
- Every pull request and push to `main` runs the required quality gates.
- Tests and optimized builds pass on GitHub-hosted Ubuntu and macOS runners.
- Workflows use a pinned Rust toolchain, locked dependencies, and read-only
  repository permissions.
- The release record distinguishes automated proof from hands-on Ghostty,
  image, mpv, session-expiry, installation, and account-flow validation.
- The `1.0.0` tag is created only from a clean commit that passed the agreed
  CI checks and manual acceptance matrix.

## Requirements

### Functional Requirements
- [ ] FR-1: Pull requests and pushes to `main` shall run continuous integration.
- [ ] FR-2: CI shall reject formatting errors and Clippy warnings.
- [ ] FR-3: CI shall run all Cargo test targets with the tracked lockfile.
- [ ] FR-4: CI shall build an optimized binary on Ubuntu and macOS.
- [ ] FR-5: CI shall verify documentation and distributable package assembly.
- [ ] FR-6: The project shall define its minimum supported Rust version and package metadata.
- [ ] FR-7: The project shall define supported installation, platform, terminal, image, and video paths.
- [ ] FR-8: Release acceptance shall preserve the open 24-hour/two-expiry and exact Ghostty checks.
- [ ] FR-9: The release procedure shall update the package version and public notes before tagging.
- [ ] FR-10: The `1.0.0` tag shall identify a commit that passed required automated and manual checks.

### Non-Functional Requirements
- Workflows must grant only the permissions required to read repository contents.
- Pull-request workflows must not receive or require release credentials.
- Superseded runs for the same branch should be cancelled to conserve runner time.
- CI commands must match commands contributors can run locally.
- Automated checks must not be presented as proof of terminal rendering, audio, or long-idle behavior.
- Release preparation must preserve public development records unless the owner explicitly changes that policy.

## Implementation Breakdown
- [ ] [[AI-IMP-004-1-ci-baseline]]: pinned, least-privilege GitHub Actions quality and platform gates (FR-1..5).

Package policy, distribution, acceptance, and tagging tickets will be cut after
the CI baseline reports real runner behavior.
