# Video Audio Implementation Brief

> **Implementation correction (2026-08-19):** This brief records the original
> ffmpeg-frame/`afplay` exploration. AI-IMP-003-6 supersedes that approach with
> a modal handoff to mpv's Kitty video output. mpv now owns HLS decoding,
> buffering, timing, controls, and synchronized audio; AT-TUI only suspends and
> restores its terminal session. The remainder is retained as historical design
> context.

## Current State
AT-TUI parses Bluesky `app.bsky.embed.video#view` embeds into `VideoRef` values containing playlist URL, thumbnail URL, alt text, CID, and aspect ratio. The media overlay treats videos as media entries after images. Thumbnail rendering uses the existing image path. Pressing Enter or `p` on a video queues experimental frame decoding through the media cache.

The current video path is intentionally visual-only. `src/media.rs` shells out to `ffmpeg` for capped low-FPS frame extraction and renders decoded frames through the terminal image path. `src/app.rs` owns overlay state, playback toggling, frame advancement timing, and background task events. There is no audio process, no audio cache, and no synchronization contract.

## Bluesky Video Shape And HLS Handling
Bluesky video views expose a playlist URL rather than a directly embedded terminal-playable file. In practice, the playlist should be treated as HLS input for `ffmpeg`/`ffprobe`. The app should avoid parsing HLS playlists by hand unless the media tool path fails; `ffmpeg` already handles redirects, variants, and segment retrieval more reliably.

For a v1 audio feature, the app should keep using the playlist URL as the source of truth and let media tooling perform demuxing. If `ffprobe` is available, use it to check whether an audio stream exists before offering audio playback. If `ffprobe` is missing, the app can optimistically attempt extraction and report failure.

## Local Tooling Options
- `ffmpeg`: already aligned with the current frame extraction path and can extract/transcode audio from HLS.
- `ffprobe`: useful for detecting audio streams, duration, codec, and failure reasons before starting playback.
- macOS `afplay`: simple local playback for extracted AAC/MP3/WAV files, but requires a temporary audio file.
- `ffplay`: convenient if installed, but not guaranteed and may open a separate playback surface.
- External browser/player: reliable fallback through the existing `u` path, but leaves the terminal experience.

## Implementation Options
1. **Extract audio to a temp file and play with `afplay` on macOS.**  
   Best fit for the current target environment. It keeps audio playback separate from terminal frame rendering and avoids streaming audio process complexity. The cost is temporary file management and loose sync.

2. **Run `ffplay` directly on the playlist.**  
   Lowest implementation effort when installed, but less controlled and not portable enough to be the default.

3. **Pipe decoded PCM to an audio crate.**  
   More native and cross-platform in theory, but it introduces Rust audio dependencies, device handling, and async process complexity. Too large for the next pass.

4. **External-player fallback only.**  
   Very reliable and simple, but it does not satisfy terminal-native audio exploration.

## Recommended V1 Path
Implement opt-in audio playback as a macOS-first companion to existing video playback:

- Detect `ffmpeg` and optionally `ffprobe`.
- When the user presses Enter/`p` on a video, continue current frame decoding behavior.
- If an audio stream is available or detection is unavailable, spawn an audio extraction task that writes a capped temporary audio file.
- Play the extracted audio with `afplay` on macOS.
- Show clear status states: audio unavailable, extracting audio, playing audio, audio failed, external fallback available.
- Do not attempt exact frame/audio sync in v1. Start audio when extraction completes and keep visual playback independent.
- Preserve `u` as the reliable external fallback for the playlist/post.

This gives AT-TUI a useful local prototype without committing to a full terminal media player architecture.

## Sync And UX Risks
- Frame extraction is already capped and low-FPS, so exact sync with full-speed audio is unlikely.
- Audio extraction may finish after visual playback starts, especially for large HLS playlists.
- `afplay` process control is basic; pause/seek integration should be deferred.
- Terminal users may be in tmux/zellij or remote shells where local audio is undesirable.
- Status text must make audio behavior explicit so a silent failure does not look like a frozen overlay.

## macOS And Ghostty Constraints
Ghostty helps with image rendering but does not provide audio. Audio must go through OS facilities or an external player. For the current primary environment, `afplay` is a reasonable default once an audio file exists. The implementation should check tool availability at runtime and avoid failing the video overlay when audio tooling is missing.

## Future Implementation Checklist
- [ ] Add audio availability state to video media cache entries.
- [ ] Add tool detection for `ffmpeg`, optional `ffprobe`, and macOS `afplay`.
- [ ] Add an audio extraction job that consumes the video playlist URL and writes a temp audio file.
- [ ] Add an audio playback job that invokes `afplay` for extracted audio on macOS.
- [ ] Add status updates for extracting, playing, unavailable, and failed states.
- [ ] Add cleanup for temporary audio files.
- [ ] Keep `u` external-open behavior available from the media overlay.
- [ ] Unit-test audio state transitions and missing-tool fallbacks.
- [ ] Unit-test command construction without invoking real media tools.
- [ ] Manually verify with a Bluesky HLS video that has audio, a video without audio, and a system without `ffprobe`.
