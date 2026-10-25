---
node_id: AI-IMP-003-6
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - media
  - video
  - performance
kanban_status: in-progress
depends_on:
  - AI-IMP-003-5
parent_epic: [[AI-EPIC-003-trustworthy-runtime-interaction-foundation]]
confidence_score: 0.9
date_created: 2026-08-17
date_completed:
---

# AI-IMP-003-6-progressive-video-events

## Frame extraction is not real video playback
The experimental path asks ffmpeg to write 120 JPEGs, waits for the complete batch, decodes every frame into memory, and then advances terminal images on an application timer. It is slow to start, stuttery, silent, and duplicates work already solved by a media player. Done state: selecting a Bluesky video hands the terminal to mpv's Kitty video output for native HLS decoding, timing, controls, and synchronized audio, then reliably restores AT-TUI when playback exits.

### Out of Scope
An in-process decoder, bordered or inline playback inside the Ratatui layout, mpv installation management, custom audio synchronization, remote-terminal audio routing, and optimized video for terminals without Kitty graphics.

### Design/Approach
Use a modal terminal-ownership handoff. AT-TUI leaves raw mode, mouse capture, and its alternate screen before launching mpv directly on the Bluesky HLS playlist. mpv owns Kitty rendering, shared-memory transfer, audio, buffering, seeking, pause, and playback input in its own alternate screen. When mpv exits, AT-TUI restores its terminal modes, clears stale pixels, and redraws. Keep thumbnail loading in the existing image scheduler, retain `u` as the external fallback, and report a clean status when mpv is unavailable.

### Files to Touch
`src/video_player.rs`: mpv discovery and safe command construction.
`src/app.rs`: playback request and terminal suspend/restore lifecycle.
`src/media.rs`: remove obsolete frame decoding and retain thumbnail/placeholder rendering.
`src/media_scheduler.rs`: remove obsolete video-frame jobs.
`src/ui.rs`: render video thumbnails without decoder state.
`src/lib.rs`: export the player module.
`README.md`: document playback behavior and optional dependency.
`RAG/spikes/video-audio-implementation-brief.md`: record the architecture correction.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Discover mpv from an explicit environment override, `PATH`, standard Homebrew prefixes, or the macOS app bundle.
- [x] Construct a direct HLS command with `--vo=kitty`, Kitty shared memory, and `--` before the untrusted playlist URL.
- [x] Suspend raw mode, mouse capture, and AT-TUI's alternate screen before starting mpv.
- [x] Restore terminal modes and redraw after normal exit, nonzero exit, or a playback spawn failure.
- [x] Let mpv own audio, timing, buffering, pause, seek, and quit input.
- [x] Remove the all-at-once JPEG extraction, decoded-frame cache, and frame timer.
- [x] Preserve video thumbnail loading and the `u` external fallback.
- [x] Report a clean missing-mpv status without preventing AT-TUI startup.
- [x] Unit-test command construction and playback request state without real network media.
- [ ] Perform a hands-on Ghostty HLS check covering playback, audio, pause/seek, `q`, and restored TUI input.
- [x] Run the full validation gate.

### Acceptance Criteria
**Scenario:** A video is selected in Ghostty.
**GIVEN** mpv with Kitty output is installed.
**WHEN** the user presses Enter or `p`.
**THEN** mpv begins actual HLS playback with synchronized audio and native playback controls rather than waiting for a JPEG batch.

**Scenario:** Playback ends or fails to start.
**GIVEN** AT-TUI handed terminal ownership to mpv.
**WHEN** mpv exits normally, exits nonzero, or cannot spawn.
**THEN** AT-TUI restores raw mode, mouse capture, alternate-screen rendering, and keyboard input without requiring an app restart.

**Scenario:** mpv is unavailable.
**GIVEN** no supported mpv executable can be discovered.
**WHEN** the user requests playback.
**THEN** AT-TUI remains usable, explains how to install mpv, and keeps `u` available as an external fallback.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
The local Homebrew formula installed successfully, but Homebrew could not create its normal `mpv` link because a stale `stolendata-mpv` cask owns the mpv manpage path. AT-TUI discovers `/opt/homebrew/opt/mpv/bin/mpv` directly, so playback does not require overwriting or removing the owner's existing package state.

A direct PTY spike against a live Bluesky HLS playlist exited successfully and emitted Kitty graphics output. That proves the mpv/HLS/Kitty command path, but a visual and audio check in the owner's actual Ghostty session remains required before completion.

The full gate passed with 144 library tests, one CLI integration test, formatting clean, Clippy warnings denied, all targets checked, and no whitespace errors.
