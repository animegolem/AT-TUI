---
node_id: AI-IMP-001-3
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - video
  - spike
kanban_status: completed
depends_on: [[AI-EPIC-001-at-tui-public-readiness-polish]]
parent_epic: [[AI-EPIC-001-at-tui-public-readiness-polish]]
confidence_score: 0.78
date_created: 2026-05-22
date_completed: 2026-08-19
---

# AI-IMP-001-3-video-audio-spike

## Video Audio Implementation Brief
AT-TUI has experimental terminal video frame decoding, but audio behavior is not defined. Implementing audio directly is larger than the current polish pass because it involves local tool availability, playback process control, and sync tradeoffs. This spike produces the handoff artifact needed to start the next video implementation push.

### Out of Scope
No audio playback code, no new dependencies, no terminal control changes, and no UI controls beyond documenting proposed future behavior.

### Design/Approach
Inspect the existing video path and produce `RAG/spikes/video-audio-implementation-brief.md`. The brief must describe current data flow, HLS playlist handling, local media tooling assumptions, playback options, sync risks, and a recommended v1 implementation path. It should end with a concrete follow-up implementation checklist that can be copied into a future AI-IMP.

### Files to Touch
`src/media.rs`: read-only reference for current video frame path.  
`src/app.rs`: read-only reference for overlay/playback controls.  
`RAG/spikes/video-audio-implementation-brief.md`: spike artifact.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Map the current video overlay and frame-decoding path.
- [x] Document how Bluesky video playlist URLs are represented and consumed.
- [x] Document required/optional local tools: `ffmpeg`, `ffprobe`, and platform audio players.
- [x] Compare audio extraction plus local playback, external-player delegation, and no-audio fallback.
- [x] Identify frame/audio sync risks and acceptable v1 compromises.
- [x] Cover macOS and Ghostty constraints explicitly.
- [x] Recommend one v1 implementation path.
- [x] Include a future implementation checklist with test and manual QA items.

### Acceptance Criteria
**Scenario:** A future implementer starts video audio work.  
**GIVEN** the spike artifact exists.  
**WHEN** they read the recommended path and checklist.  
**THEN** they can begin implementation without re-discovering current media flow, local tool assumptions, or playback tradeoffs.

### Issues Encountered
The spike produced `RAG/spikes/video-audio-implementation-brief.md`. The later
AI-IMP-003-6 implementation replaced the brief's ffmpeg and `afplay` proposal
with a modal mpv Kitty handoff. The brief now labels that recommendation as
historical.
