---
node_id: AI-IMP-003-5
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - media
  - performance
  - lifecycle
kanban_status: in-progress
depends_on:
  - AI-IMP-003-2
parent_epic: [[AI-EPIC-003-trustworthy-runtime-interaction-foundation]]
confidence_score: 0.84
date_created: 2026-08-17
date_completed:
---

# AI-IMP-003-5-prioritized-media-scheduler

## Speculative media work is unbounded and cannot yield to visible work
Nearby-thumbnail prefetch materially improved speed, but each eligible URL spawns immediately. Rapid navigation can create many downloads/decodes, speculative work has the same priority as the selected overlay, loading entries may be evicted before completion, failures remain sticky for the process lifetime, and the disk cache has no cleanup bound. Done state: one scheduler bounds and prioritizes media work, cancels obsolete speculation, deduplicates in-flight requests, supports controlled retry, and keeps disk use bounded.

### Out of Scope
Inline timeline image layout, audio, remote cache synchronization, image upload, and progressive video frames (AI-IMP-003-6).

### Design/Approach
Introduce media job identity, priority, and lifecycle separate from rendered cache state. Use small explicit concurrency limits for download and CPU decode work. Priority order is visible selected media, selected-post thumbnail, nearby thumbnails, then full-size speculation. Reprioritize an existing job rather than duplicate it. Cancel or discard speculative jobs invalidated by view generation. Record retryable versus permanent failures and add age/size-based disk cleanup with deterministic tests.

### Files to Touch
`src/media.rs`: cache state integration and disk cleanup.
`src/media/scheduler.rs` or `src/media_scheduler.rs`: bounded priority queue and job lifecycle.
`src/app.rs`: submit priorities, consume completions, cancel obsolete speculation.
`src/lib.rs`: module export if needed.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Define stable media job identity and priority classes.
- [x] Bound simultaneous HTTP downloads and CPU decodes independently.
- [x] Deduplicate queued and in-flight work by media identity.
- [x] Reprioritize visible media ahead of existing speculative work.
- [x] Cancel or ignore prefetch invalidated by navigation generation.
- [x] Keep in-flight state outside the decoded-image LRU eviction path.
- [x] Classify media failures and permit bounded retry for transient errors.
- [x] Add deterministic disk-cache age/size cleanup while preserving current-session entries.
- [x] Add scheduler tests for bounds, ordering, reprioritization, cancellation, retry, and cleanup.
- [ ] Perform a hands-on rapid-scroll/media-open test in Ghostty.
- [x] Run the full validation gate.

### Acceptance Criteria
**Scenario:** The user opens media while prefetch is busy.
**GIVEN** all media worker slots are occupied by nearby thumbnails.
**WHEN** the selected post's overlay requests its image.
**THEN** that job becomes the next eligible work and no duplicate request is created.

**Scenario:** The user scrolls rapidly.
**GIVEN** several generations of speculative prefetch have been queued.
**WHEN** selection moves beyond their relevance window.
**THEN** obsolete queued work is cancelled and total work remains within configured bounds.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
Media loading now passes through one priority queue keyed by media kind and source URL. The queue has separate four-download and two-decode semaphores, retains physical slots until cancelled tasks actually complete, reprioritizes a cancelled in-flight request if the same URL becomes visible again, and permits one automatic retry for classified transient failures. Loading identities live in dedicated sets rather than the decoded LRU. Explicit user opens can retry a previously sticky failure, while background prefetch cannot loop on it.

The disk cache is pruned at startup and after successful writes using a 30-day age limit and a 256 MiB size target. Entries read or written during the current process are protected, and age/size behavior is covered with deterministic temporary-directory tests. Media HTTP requests now have a 20-second timeout.

A live saved-account PTY pass used halfblock rendering, rapidly moved from row 1 to row 41, returned to a two-image post, opened the media overlay, observed image 1 load and render, switched to image 2, observed it load and render, then exited cleanly. The available desktop-control layer explicitly refused access to `com.mitchellh.ghostty` for safety reasons, so this equivalent terminal smoke test does not satisfy the ticket's exact Ghostty checkbox; that one manual check remains open. The full gate passed with 142 unit tests, one CLI integration test, warnings denied, a clean build, and no whitespace errors.
