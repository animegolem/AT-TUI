# Agent Notes

## RAG Ticket Workflow

- Keep planning work in `RAG/AI-EPIC`, `RAG/AI-IMP`, and `RAG/spikes`.
- Treat `RAG/INDEX.md` as generated output. Do not edit it by hand.
- After creating or changing RAG tickets, run:

```sh
bash RAG/scripts/generate-index.sh
```

- The generator scans tracked files, rewrites `RAG/INDEX.md`, normalizes a few frontmatter aliases, and adds `parent_epic` backlinks to implementation tickets when it can infer them from `depends_on`.
- This checkout uses the tracked `.githooks/pre-commit` hook. The hook runs the generator and stages `RAG/INDEX.md`, `RAG/AI-EPIC`, and `RAG/AI-IMP` so committed ticket changes include the refreshed index.
- To enable the tracked hook in a fresh clone, run:

```sh
git config core.hooksPath .githooks
```

## Codebase Review

AT-TUI is organized around a small set of large modules:

- `src/main.rs` owns CLI parsing and dispatch for account commands or launching the TUI.
- `src/config.rs` owns local app-password account/session persistence.
- `src/api.rs` wraps Bluesky XRPC calls, session refresh, and record writes.
- `src/model.rs` normalizes Bluesky JSON into renderable app models.
- `src/navigation.rs` owns view stack state, selection, scrolling, and render cache metadata.
- `src/app.rs` owns the event loop, background task orchestration, key actions, overlays, and write flows.
- `src/ui.rs` renders the list, statusline, overlays, post rows, profile rows, and notification rows.
- `src/media.rs` owns terminal image/video cache state, image protocol setup, and experimental ffmpeg frame decoding.

The current split is workable for a prototype, but several files are beyond the point where future feature work will stay easy:

- `src/app.rs` is the main pressure point. It should eventually split into action/keymap handling, task/event handling, composer/write flows, and feed/view loading.
- `src/model.rs` mixes response parsing for posts, preferences, profiles, notifications, links, and media. Split by domain once follow/unfollow or notification browsing grows.
- `src/ui.rs` mixes layout, row rendering, overlays, and statusline rendering. Split row renderers and overlays before adding more surfaces.
- `src/media.rs` combines image cache, video decode, and Ratatui rendering. Split video support when audio/playback work starts.
- `RAG/scripts/generate-index.sh` is long but acceptable for now because it is an isolated repo utility. If it grows, split extraction/index rendering helpers into smaller shell scripts or replace it with a tiny Rust/Python tool.

## Practical Defaults

- Prefer small, ticketed changes through the RAG workflow now that the app has multiple active surfaces.
- Keep narrow, single-column behavior as the default UI contract.
- Preserve background-task behavior for network, media, browser, and write actions; key handling should not await slow work.
- Keep app-password auth until OAuth is planned as its own implementation ticket.
