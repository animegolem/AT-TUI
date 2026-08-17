---
node_id: AI-IMP-003-6
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - media
  - video
  - performance
kanban_status: planned
depends_on:
  - AI-IMP-003-5
parent_epic: [[AI-EPIC-003-trustworthy-runtime-interaction-foundation]]
confidence_score: 0.78
date_created: 2026-08-17
date_completed:
---

# AI-IMP-003-6-progressive-video-events

## Video waits for complete decoding before the UI can play
The experimental video job runs ffmpeg in blocking work and returns a complete `Vec<DynamicImage>`. The user sees no frames until extraction finishes, decoded memory arrives in one burst, and closing the overlay does not define ownership of the external process. Done state: video frame batches become visible progressively, decoder and memory limits are explicit, and obsolete playback is cancelled cleanly.

### Out of Scope
Audio extraction/playback, A/V synchronization, a general media-player framework, HLS parsing in Rust, and background prefetch of full videos.

### Design/Approach
Build on the media scheduler and the existing `RAG/spikes/video-audio-implementation-brief.md`. Keep ffmpeg responsible for HLS. Emit frame-ready events in small batches as files become available or through a bounded decoder channel; begin playback after a minimum buffer, continue filling up to a cap, and apply backpressure. Associate each decoder with media job/generation identity. On close, replacement, timeout, or shutdown, terminate the owned child and reject late frames.

### Files to Touch
`src/media.rs`: progressive video state and bounded frame buffer.
`src/media/scheduler.rs` or equivalent: decoder ownership/cancellation.
`src/app.rs`: frame events and playback lifecycle.
`src/ui.rs`: buffering/progress state if needed.
`RAG/spikes/video-audio-implementation-brief.md`: record any corrected implementation assumptions.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [ ] Define progressive video events for started, frames-ready, completed, failed, and cancelled.
- [ ] Replace all-at-once frame return with bounded batches or a bounded channel.
- [ ] Start playback after a tested minimum buffer instead of complete decode.
- [ ] Enforce frame count and decoded-memory limits with backpressure or dropping policy.
- [ ] Tie ffmpeg child ownership to the media job and view generation.
- [ ] Terminate the child on overlay close, video replacement, timeout, and app shutdown.
- [ ] Ignore late frames from obsolete decoder generations.
- [ ] Unit-test state transitions and command construction without real network media.
- [ ] Perform a hands-on HLS video check with visible time-to-first-frame evidence.
- [ ] Run the full validation gate.

### Acceptance Criteria
**Scenario:** A video takes several seconds to decode fully.
**GIVEN** ffmpeg begins producing valid frames.
**WHEN** the minimum playback buffer is ready.
**THEN** the overlay starts playing before complete extraction and remains within its memory cap.

**Scenario:** Playback closes during decode.
**GIVEN** an owned ffmpeg process is still producing frames.
**WHEN** the user closes or replaces the media overlay.
**THEN** the child is terminated and late frame events cannot repopulate video state.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
