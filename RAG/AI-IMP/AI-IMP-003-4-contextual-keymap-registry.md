---
node_id: AI-IMP-003-4
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - keymap
  - interaction
  - architecture
kanban_status: planned
depends_on:
  - AI-IMP-003-2
parent_epic: [[AI-EPIC-003-trustworthy-runtime-interaction-foundation]]
confidence_score: 0.9
date_created: 2026-08-17
date_completed:
---

# AI-IMP-003-4-contextual-keymap-registry

## Key behavior has two sources of truth
Normal-mode keys map into `Action`, but menu, media, link-picker, composer, and confirmation overlays match keys directly inside `App::handle_key`. Help and README text are maintained separately. Adding or changing bindings therefore requires finding multiple dispatch and documentation sites. Done state: every interactive context uses one typed binding registry, and the help overlay is rendered from the same metadata.

### Out of Scope
User-configurable key files, chord sequences, mouse remapping, changing the agreed bindings, and a command palette.

### Design/Approach
Create explicit input contexts and a binding record containing key pattern, action, display label, and help grouping. Keep action execution in `App`; the registry only translates input. Represent composer character insertion separately from commands so ordinary text entry stays direct and predictable. Derive in-app help from registered bindings and add a consistency check for the README key table or update it through a small deterministic generator.

### Files to Touch
`src/keymap.rs`: contexts, actions/bindings, lookup, help metadata, tests.
`src/app.rs`: context selection and action execution.
`src/ui.rs`: render registry-derived help.
`src/lib.rs`: module export.
`README.md`: synchronized key documentation.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [ ] Define input contexts for normal, menu, media, link picker, composer, and confirmation states.
- [ ] Define typed actions without coupling binding lookup to application mutation.
- [ ] Move all command-key translation into the registry.
- [ ] Preserve literal composer text entry and modifier behavior.
- [ ] Render help sections from registry metadata.
- [ ] Synchronize README keys from the same inventory or verify them deterministically.
- [ ] Reject duplicate/conflicting bindings within a context in tests.
- [ ] Preserve the full current keymap and retired-key expectations.
- [ ] Perform hands-on checks in every context, including Escape, `q`, Enter, and Ctrl-S.
- [ ] Run the full validation gate.

### Acceptance Criteria
**Scenario:** A binding changes.
**GIVEN** a developer changes one registry entry.
**WHEN** the app renders help and the documentation check runs.
**THEN** dispatch and displayed help agree without editing a second in-code key list.

**Scenario:** The same key has contextual meanings.
**GIVEN** Space opens media in normal mode and closes the media overlay inside that overlay.
**WHEN** Space is pressed in each context.
**THEN** the registry resolves the correct contextual action with no global conflict.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
