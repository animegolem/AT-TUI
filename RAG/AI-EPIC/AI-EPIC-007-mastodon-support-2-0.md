---
node_id: AI-EPIC-007
tags:
  - EPIC
  - AI
  - at-tui
  - mastodon
  - "2.0"
date_created: 2026-10-25
date_completed:
kanban_status: deferred
AI_IMP_spawned:
---

# AI-EPIC-007-mastodon-support-2-0

> **Deferred (owner, 2026-10-25).** Stubbed so the scope isn't lost, not planned. AT Protocol stays the only network for now. Revisit only on the owner's word. A second network is a 2.0-sized change.

## Problem Statement/Feature Scope
at-tui is Bluesky-only. Mastodon is the other open network where a terminal client is legitimate. X is ruled out: its Developer Agreement (last updated 2026-04-27) forbids building "a substitute or similar service or product to the X Applications", and pay-per-use reads (about $0.005 per post) would cost roughly $15/hour at the app's 60 s refresh. The open question is demand, not feasibility. Part of the fediverse is hostile to AI-assisted projects, so the audience this serves may not adopt it.

## Proposed Solution(s)
Scope estimate from the 2026-10-25 review: about 1.5–2.5 weeks of focused work, +3–4k lines on a ~10k-line codebase.

1. **Network abstraction (largest piece).** `FeedItem`/`PostRef`, viewer state (like/repost record URIs), `Session`/`AccountConfig` and `BskyClient` all assume ATProto. Introduce a per-network backend (enum or trait), opaque post IDs, and per-network accounts in `accounts.json`, with migration of existing files.
2. **Mastodon client.** Per-instance app registration plus an OAuth code flow from the CLI (`at-tui login --mastodon <instance>`; print the URL, paste the code). Tokens don't rotate, so the refresh logic doesn't apply. Endpoints: home/local/federated/list/hashtag timelines, `statuses/:id/context`, notifications (grouped where the server supports it), accounts, favourite/reblog/follow, statuses create/delete. Pagination via `Link` headers (`max_id`), not cursor fields.
3. **Parsing.** Status `content` is HTML; convert to text, keeping links and mentions. Also custom `:shortcode:` emoji, polls (display only), visibility (public/unlisted/private/direct), and media with `sensitive`.
4. **UI.** Content warnings (`spoiler_text`) collapsed by default; this is core etiquette, not optional. Shares work with EPIC-003 proposal 19 (content labels). Add a visibility marker, composer visibility and CW fields, and the instance character limit (default 500; URLs count as 23).
5. **Threads.** `context` returns flat ancestors/descendants; rebuild the tree from `in_reply_to_id` for the existing tree view.
6. **Test infrastructure.** Extend `demo/mock_server.py` with a Mastodon surface, and add fixtures and specs in the existing style.

## Path(s) Not Taken
X/Twitter (forbidden by its terms, and too expensive to read). Scraping or private-API access to any network.

## Success Metrics
Defined only if the epic is revived.

## Requirements
To be cut into AI-IMPs if and when this is picked up. Suggested order: 1 → 2 → 3 → 5 → 4 → 6, with 4's CW handling done before any public mention.

## Implementation Breakdown
None. Deferred.
