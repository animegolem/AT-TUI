---
node_id: AI-IMP-001-2
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - bluesky
  - follow
kanban_status: completed
depends_on: [[AI-EPIC-001-at-tui-public-readiness-polish]]
parent_epic: [[AI-EPIC-001-at-tui-public-readiness-polish]]
confidence_score: 0.82
date_created: 2026-05-22
date_completed: 2026-05-23
---

# AI-IMP-001-2-follow-unfollow-accounts

## Keyboard Follow And Unfollow
AT-TUI can browse posts, profiles, and notifications, but it cannot yet mutate graph relationships. This ticket adds a provisional keyboard follow/unfollow path so a user can act on accounts encountered in the timeline, notifications, and profile views.

### Out of Scope
Mouse follow buttons, bulk follow management, list memberships, blocking/muting, OAuth migration, and permanent keymap customization are not part of this ticket.

### Design/Approach
Use provisional `w` as the normal-mode follow toggle until keymap-file support exists. Read `viewer.following` from profile/author viewer state where Bluesky returns it, and preserve that follow record URI in the render model. Follow creates an `app.bsky.graph.follow` record for the target DID; unfollow deletes the existing follow record URI. Guard against following the active account. Update visible matching items/profile state only after successful writes, and show transient statuses for success/failure.

### Files to Touch
`src/api.rs`: create follow record helper.  
`src/model.rs`: viewer follow state on profiles/authors.  
`src/app.rs`: `w` action, background write task, local state updates.  
`src/ui.rs`: profile follow-state indicator and menu key help.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Parse author/profile `viewer.following` into a follow record URI.
- [x] Add `BskyClient::create_follow(subject_did)` using `com.atproto.repo.createRecord` with collection `app.bsky.graph.follow`.
- [x] Reuse existing record deletion for unfollow.
- [x] Add provisional normal-mode `w` action and menu help text.
- [x] Resolve the follow target from selected post author, selected notification author, or active profile.
- [x] Prevent following/unfollowing the active account and show a transient status.
- [x] Queue follow/unfollow writes in the background.
- [x] Update matching visible author/profile follow state only after success.
- [x] Show profile follow status in the profile header.
- [x] Add tests for record construction, self-follow guard, target resolution, successful toggle updates, and failed write behavior.

### Acceptance Criteria
**Scenario:** User follows a profile from the profile view.  
**GIVEN** the profile is not already followed.  
**WHEN** the user presses `w`.  
**THEN** AT-TUI creates a follow record in the background.  
**AND** the profile header updates only after success.

**Scenario:** User unfollows an account from a selected post.  
**GIVEN** the selected post author has a `viewer.following` record URI.  
**WHEN** the user presses `w`.  
**THEN** AT-TUI deletes that follow record.  
**AND** visible matching author state is cleared after success.

**Scenario:** User selects their own account.  
**GIVEN** the follow target DID matches the active session DID.  
**WHEN** the user presses `w`.  
**THEN** no API write is queued.  
**AND** the statusline shows that self-follow is not allowed.

### Issues Encountered
Implementation is complete in the current working tree and validated with `cargo fmt -- --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, and `git diff --check`; it has not been committed yet.
