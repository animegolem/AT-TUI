---
node_id: AI-EPIC-008
tags:
  - EPIC
  - AI
  - at-tui
  - backlog
  - "1.1"
date_created: 2026-10-25
date_completed:
kanban_status: backlog
AI_IMP_spawned:
---

# AI-EPIC-008-1-1-carryover

## Problem Statement/Feature Scope
1.0 shipped from the release-review line (EPIC-006). Only part of the parallel `codex/ci-baseline` line (EPIC-003/005) was ported: CI, mpv playback, the CLI test, and cancelling in-flight loads when going back. This epic lists what was deliberately left out of 1.0, so none of it is lost. It's a backlog, not a plan; cut IMPs only on the owner's word.

## Proposed Solution(s)
**From codex/ci-baseline (code is reachable at `c1713c7`; tickets cited):**
- Keymap registry with help/README generation (003-4). It's structure only, and main's bindings stay authoritative (Esc/`?` menu, Tab switcher, `^W`/`^U`/`^E`).
- Prioritised media scheduler with visible-demand promotion, plus atomic cache publication and coalesced maintenance (003-5, 005-3, 005-4).
- View-instance identities and reload tokens (005-1/005-2 beyond the cancel-on-back slice); task lifecycles/timeouts per context (003-2).
- Layout reuse on append (005-6); profile header scrolling with the view buffer (003-8); runtime diagnostics (003-7).
- An in-process lock around accounts-file read-modify-write (003-3); mock-HTTP tests for the refresh paths (003-1).

**From EPIC-006 proposals still open:** 17 quit guard for in-flight writes, 18 viewport-relative paging, 19 content labels (do this before any wider announcement), 20 composer cursor movement.

**Acceptance gaps from EPIC-004:** a 24-hour/two-expiry idle run, and visual checks of images and mpv in Ghostty and kitty.

**Known issues:**
- After the image-protocol query inside tmux, the first keystroke is swallowed. This predates 1.0; the August binary does it too.
- A post taller than the screen can't be scrolled within.
- Launch fails hard when offline.
- Login doesn't resolve a self-hosted PDS from the handle (`--service` is needed).
- A quoted post opened with `e` lacks viewer state, so `f` there can double-like.

## Path(s) Not Taken
Mastodon is a 2.0 item; see [[AI-EPIC-007-mastodon-support-2-0]].

## Success Metrics
Defined per IMP when cut.

## Requirements
To be cut into AI-IMPs.

## Implementation Breakdown
None yet.
