---
node_id: AI-IMP-002-6
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - posting
  - facets
kanban_status: planned
depends_on: [[AI-IMP-002-4]]
parent_epic: [[AI-EPIC-002-at-tui-stabilization-daily-driver]]
confidence_score: 0.8
date_created: 2026-07-06
date_completed:
---

# AI-IMP-002-6-posting-correctness

## Summary of Issue #1
Posts written from at-tui are second-class: no facets are generated, so links and @mentions render as dead plain text in every other client; length is validated by `chars().count()`, rejecting emoji-heavy posts the API accepts (300 graphemes); and there is no way to delete a post you regret. Done state: outgoing posts carry link and mention facets, the composer counts graphemes, and `d` deletes your own selected post after confirmation.

### Out of Scope
Hashtag facets, image upload, post editing, deleting likes/reposts (already exists), threadgates.

### Design/Approach
Facets: a `facets_for_text` builder in api.rs — linkify finds URLs (byte offsets are already UTF-8 byte indices, matching the lexicon), a scanner finds `@handle.tld` mentions validated by a light handle shape check; each mention resolves via new `com.atproto.identity.resolveHandle` call, silently skipped on failure. `create_post` attaches the facets array when non-empty. Graphemes: add `unicode-segmentation`; count with `graphemes(true)` in `submit_composer` and the ui counter. Delete: `d` on a selected post whose `author_did` matches the session DID opens a new `Overlay::ConfirmDelete { uri, .. }`; `y` runs `delete_record_uri` as a write task, and on success the post is removed from all views (a `NavigationStack::retain_items` helper); any other key cancels.

### Files to Touch
`src/api.rs`: `facets_for_text`, `resolve_handle`, `create_post` facets.
`src/app.rs`: grapheme validation, `d` action + confirm overlay + write result.
`src/ui.rs`: confirm overlay render, grapheme counter.
`src/navigation.rs`: remove-item helper.
`Cargo.toml`: `unicode-segmentation`.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [ ] `facets_for_text` emits link facets with correct byte ranges (multibyte-text test).
- [ ] Mention facets resolve handles; unresolvable mentions skipped without failing the post.
- [ ] `create_post` includes facets when present.
- [ ] Grapheme-based length check in composer submit and counter display.
- [ ] `d` opens confirm overlay only for own posts; `y` deletes, others cancel.
- [ ] Successful delete removes the post from all stacked views and shows status.
- [ ] Tests: facet byte offsets, grapheme edge case (emoji ZWJ), delete guard for foreign posts.
- [ ] Gate: fmt, test, clippy -D warnings, build.

### Acceptance Criteria
**Scenario:** Posting a link.
**GIVEN** a composed post containing `https://example.com` after an emoji.
**WHEN** it is submitted.
**THEN** the created record carries a link facet whose byte range matches the URL exactly.

**Scenario:** Deleting own post.
**GIVEN** the selection is on the user's own post.
**WHEN** `d` then `y` is pressed.
**THEN** the record is deleted and the post disappears from every open view.
**AND** pressing `d` on someone else's post shows a status and no overlay.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
