---
node_id: AI-IMP-003-3
tags:
  - IMP-LIST
  - Implementation
  - at-tui
  - auth
  - persistence
  - reliability
kanban_status: completed
depends_on:
  - AI-IMP-003-1
parent_epic: [[AI-EPIC-003-trustworthy-runtime-interaction-foundation]]
confidence_score: 0.92
date_created: 2026-08-17
date_completed: 2026-08-18
---

# AI-IMP-003-3-atomic-session-persistence

## Session saves can destroy recoverable account state
`SessionStore::save` and `save_account` replace any load/parse failure with `AccountConfig::default`, while `save_config` truncates `accounts.json` before writing the replacement. A malformed read or interrupted write can therefore turn a recoverable error into lost accounts. The migrated legacy `session.json` also remains as a stale credential source. Done state: configuration updates fail closed, commit atomically, preserve the prior readable file, and complete legacy migration without a stale re-import path.

### Out of Scope
OS keychain integration, encrypted-at-rest token storage, OAuth, cloud account sync, and changes to CLI account semantics.

### Design/Approach
Propagate `load_config` errors instead of defaulting during updates. Serialize the complete replacement, write it with restrictive permissions to a sibling temporary file, flush it, and atomically rename it over `accounts.json`; preserve or create a bounded backup when practical. Make migration idempotent and remove or archive the legacy file only after the new configuration is durably readable. Avoid logging credential contents.

### Files to Touch
`src/config.rs`: safe read-modify-write, atomic commit, migration cleanup, tests.
`src/main.rs`: only if improved recovery errors need CLI context.

### Implementation Checklist
<CRITICAL_RULE>
Before marking an item complete on the checklist MUST **stop** and **think**. Have you validated all aspects are **implemented** and **tested**?
</CRITICAL_RULE>

- [x] Remove `unwrap_or_default` from credential update paths.
- [x] Write serialized configuration to a restrictive sibling temporary file.
- [x] Flush and atomically rename the completed file over `accounts.json`.
- [x] Preserve the original configuration if serialization, write, flush, or rename fails.
- [x] Define and implement a bounded backup/recovery policy.
- [x] Complete legacy migration only after the new configuration reloads successfully.
- [x] Prevent a stale legacy session from being imported after successful migration.
- [x] Add tests for malformed input, simulated interrupted write, permissions, migration idempotence, and multi-account refresh updates.
- [x] Verify CLI `login`, `accounts`, `switch`, `session`, and `logout` against temporary stores.
- [x] Run the full validation gate.

### Acceptance Criteria
**Scenario:** Existing account configuration is malformed.
**GIVEN** `accounts.json` cannot be parsed.
**WHEN** a refresh attempts to save rotated tokens.
**THEN** the save returns an error and does not replace the file with an empty account list.

**Scenario:** A save is interrupted.
**GIVEN** a readable multi-account configuration exists.
**WHEN** replacement writing fails before atomic rename.
**THEN** the original configuration remains readable and complete.

### Issues Encountered
<!--
The comments under the 'Issues Encountered' heading are the only comments you MUST not remove
This section is filled out post work as you fill out the checklists.
You SHOULD document any issues encountered and resolved during the sprint.
You MUST document any failed implementations, blockers or missing tests.
-->
Credential updates now serialize before touching disk, share an in-process update lock across `SessionStore` clones, write and flush a mode-0600 sibling temporary file, atomically rename it, and sync the containing directory. A single `accounts.json.bak` preserves the immediately previous readable configuration; it is overwritten on the next successful update, is never loaded automatically over a malformed primary, and is removed by a full store clear. `AT_TUI_ACCOUNTS_FILE` provides an explicit isolated-store path for testing and manual recovery work.

Legacy migration now reloads and compares the committed account configuration before removing `session.json`. Any later successful load also removes a leftover legacy file, so interrupted cleanup cannot re-import stale credentials. The binary integration test exercised `login` against a local mock XRPC service, then `accounts`, `switch`, `session`, and `logout` against a temporary store without touching live Bluesky state. The full gate passed with 129 unit tests and one CLI integration test; no blockers or missing ticket-scoped tests remain.
