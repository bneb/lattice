# BRIEFING — 2026-07-11T15:11:08-07:00

## Mission
Eradicate AI slop, hyperbole, and legacy artifacts from /Users/kevin/projects/lettuce, ensuring correctness and test pass.

## 🔒 My Identity
- Archetype: sub_orch
- Roles: orchestrator, user_liaison, human_reporter, successor
- Working directory: /Users/kevin/projects/lattice/.agents/sub_orch_lettuce
- Original parent: parent
- Original parent conversation ID: c6ca45a3-f366-44f9-a89d-17fc7dc03fef

## 🔒 My Workflow
- **Pattern**: Project
- **Scope document**: /Users/kevin/projects/lattice/.agents/sub_orch_lettuce/SCOPE.md
1. **Decompose**: Decompose the repository audit into manageable files/modules or run a single audit iteration since it's a repository audit.
2. **Dispatch & Execute** (pick ONE):
   - **Direct (iteration loop)**: Spawn Explorers, Worker, Reviewers, Challengers, Auditor in an iterative loop.
   - **Delegate (sub-orchestrator)**: N/A
3. **On failure** (in this order):
   - Retry: nudge stuck agent or re-send task
   - Replace: spawn fresh agent with partial progress
   - Skip: proceed without (only if non-critical)
   - Redistribute: split stuck agent's remaining work
   - Redesign: re-partition decomposition
   - Escalate: report to parent (sub-orchestrators only, last resort)
4. **Succession**: Spawn successor if spawn count >= 16 and all subagents are complete.
- **Work items**:
  1. Initialize files and schedule timers [in-progress]
  2. Scan and audit lettuce repository [pending]
  3. Verify and commit changes [pending]
  4. Handoff to parent [pending]
- **Current phase**: 1
- **Current focus**: Initialize files and schedule timers

## 🔒 Key Constraints
- Repository to audit: /Users/kevin/projects/lettuce
- Commit and push to main branch of /Users/kevin/projects/lettuce
- NEVER add Co-Authored-By or Signed-off-by trailers attributing work to Claude/Anthropic
- Keep changes minimal and reviewable
- Max 32 non-blank lines per function, max 500 lines per file, max 3 nesting (for any changes)
- Never rewrite working code without permission; perform clean-ups (slop, hyperbole, legacy artifacts)

## Current Parent
- Conversation ID: c6ca45a3-f366-44f9-a89d-17fc7dc03fef
- Updated: not yet

## Key Decisions Made
- [TBD]

## Team Roster
| Agent | Type | Work Item | Status | Conv ID |
|-------|------|-----------|--------|---------|
| Explorer 1 | teamwork_preview_explorer | Scan lettuce source code A-R | pending | e0645d64-72e8-46de-823d-b996b30af8e8 |
| Explorer 2 | teamwork_preview_explorer | Scan lettuce source code S-Z | pending | 00464fc4-0d52-4a5d-b445-9d7b31c70c25 |
| Explorer 3 | teamwork_preview_explorer | Scan lettuce docs and others | failed | 2a854e23-2f16-4579-be91-47b550d85871 |
| Explorer 3 (Rep) | teamwork_preview_explorer | Scan lettuce docs and others | pending | 6b1ca585-4ccd-4ad9-839f-601a4e61d60e |

## Succession Status
- Succession required: no
- Spawn count: 4 / 16
- Pending subagents: e0645d64-72e8-46de-823d-b996b30af8e8, 00464fc4-0d52-4a5d-b445-9d7b31c70c25, 6b1ca585-4ccd-4ad9-839f-601a4e61d60e
- Predecessor: none
- Successor: not yet spawned

## Active Timers
- Heartbeat cron: ec3a415a-9fa3-4337-9a82-a36a27c7436e/task-11
- Safety timer: ec3a415a-9fa3-4337-9a82-a36a27c7436e/task-13
- On succession: kill all timers before spawning successor
- On context truncation: run `manage_task(Action="list")` — re-create if missing

## Artifact Index
- /Users/kevin/projects/lattice/.agents/sub_orch_lettuce/ORIGINAL_REQUEST.md — Original user request
- /Users/kevin/projects/lattice/.agents/sub_orch_lettuce/BRIEFING.md — Persistent working memory
- /Users/kevin/projects/lattice/.agents/sub_orch_lettuce/progress.md — Liveness and state checkpoint
- /Users/kevin/projects/lattice/.agents/sub_orch_lettuce/SCOPE.md — Milestone M4_lettuce details
