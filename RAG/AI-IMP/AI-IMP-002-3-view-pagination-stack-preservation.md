---
node_id: AI-IMP-002-3
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - pagination
  - navigation
kanban_status: planned
depends_on:
parent_epic: [[AI-EPIC-002-at-tui-stabilization-daily-driver]]
confidence_score: 0.85
date_created: 2026-07-06
date_completed:
---

# AI-IMP-002-3-view-pagination-stack-preservation

## Summary of Issue #1
`maybe_load_more` only fires for `ViewKind::Timeline`, so profile and notification views store cursors they never use — scrolling dead-ends at 50 items. Separately, `apply_feed_loaded` replaces the entire `NavigationStack`, so a feed switch that resolves while the user is inside a thread yanks them out (and the loading flag is set/cleared on whatever view is current, not the view being loaded). Done state: profile and notification views paginate near the bottom; a completing feed load swaps only the root view and leaves pushed views intact.

### Out of Scope
Thread pagination (depth > 8 fetches), pull-to-refresh semantics, feed switching UX changes.

### Design/Approach
Generalize pagination: near-bottom check stays in `maybe_load_more`; dispatch on the current view kind — Timeline uses the active `FeedSource` (unchanged), `Profile { actor }` calls `get_author_feed(actor, cursor)`, `Notifications` calls `list_notifications(cursor)` without a seen update. Reuse `PageLoaded` for profile pages by carrying a target discriminator, or add `ProfilePageLoaded`/`NotificationsPageLoaded` events with their own pending ids — choose the variant that keeps `apply_event` guards symmetrical (separate events preferred for clarity). Appended notification rows convert via `ViewItem::Notification`. For stack preservation, add `NavigationStack::replace_root(ViewState)`; `apply_feed_loaded` uses it and sets/clears `loading` on the root view specifically.

### Files to Touch
`src/app.rs`: pagination dispatch, new events + pending ids, `apply_feed_loaded` root replacement.
`src/navigation.rs`: `replace_root`, root accessor.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [ ] `navigation.rs`: `replace_root` preserving pushed views; unit test.
- [ ] `app.rs`: `maybe_load_more` dispatches for Profile and Notifications using stored cursors.
- [ ] `app.rs`: new page-loaded events guarded by request id and view identity (actor/kind still current).
- [ ] `app.rs`: notification pages append without re-triggering `updateSeen`.
- [ ] `app.rs`: `apply_feed_loaded` replaces root in place; loading flag targets the root view.
- [ ] Tests: profile pagination appends and updates cursor; stale page dropped; feed load keeps pushed thread view.
- [ ] Gate: fmt, test, clippy -D warnings, build.

### Acceptance Criteria
**Scenario:** Reading a prolific profile.
**GIVEN** a profile view holding 50 posts and a cursor.
**WHEN** the selection reaches five items from the bottom.
**THEN** the next page loads in the background and appends.

**Scenario:** Feed switch while reading a thread.
**GIVEN** the user pressed `]` and then opened a thread before the load finished.
**WHEN** the feed load completes.
**THEN** the thread view stays on screen and going back lands on the new feed.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
