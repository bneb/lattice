# BRIEFING — 2026-07-11T15:10:13-07:00

## Mission
Perform a comprehensive audit of all proprietary source code and documentation in /Users/kevin/projects/keuos to eradicate AI slop, hyperbole, and legacy artifacts, while ensuring correctness and test verification.

## 🔒 My Identity
- Archetype: teamwork_preview_orch
- Roles: orchestrator, user_liaison, human_reporter, successor
- Working directory: /Users/kevin/projects/lattice/.agents/sub_orch_keuos
- Original parent: parent
- Original parent conversation ID: c6ca45a3-f366-44f9-a89d-17fc7dc03fef

## 🔒 My Workflow
- **Pattern**: Project
- **Scope document**: /Users/kevin/projects/lattice/.agents/sub_orch_keuos/SCOPE.md
1. **Decompose**: Decomposed into audit, worker implementation, and review/challenge/audit phases.
2. **Dispatch & Execute**:
   - **Direct (iteration loop)**: Spawn 3 Explorers, 1 Worker, 2 Reviewers, 2 Challengers, and 1 Forensic Auditor.
3. **On failure**:
   - Retry: nudge stuck agent or re-send task
   - Replace: spawn fresh agent with partial progress
   - Skip: proceed without (only if non-critical)
   - Redistribute: split stuck agent's remaining work
   - Redesign: re-partition decomposition
   - Escalate: report to parent (sub-orchestrators only, last resort)
4. **Succession**: Self-succeed at 16 spawns, write handoff.md, spawn successor.
- **Work items**:
  1. Initialize files and timers [done]
  2. Scan and audit codebase (Explorers) [pending]
  3. Apply cleanups and test (Worker) [pending]
  4. Verify (Reviewers & Challengers) [pending]
  5. Audit integrity (Forensic Auditor) [pending]
  6. Commit, push and handoff [pending]
- **Current phase**: 1
- **Current focus**: Initializing BRIEFING.md, progress.md, and SCOPE.md.

## 🔒 Key Constraints
- NEVER edit kernel/core/, kernel/mem/, kernel/sched/ core logic.
- NEVER edit vendor/ or isodir/boot/.
- Never boot QEMU (requires interactive inspection). Run the test runner `python3 tools/run_all_tests.py` and `make kernel` to verify.
- Commit and push to main branch of /Users/kevin/projects/keuos without Co-Authored-By or Signed-off-by trailers attributing work to Claude/Anthropic.
- Never reuse a subagent after it has delivered its handoff — always spawn fresh.

## Current Parent
- Conversation ID: c6ca45a3-f366-44f9-a89d-17fc7dc03fef
- Updated: not yet

## Key Decisions Made
- [TBD]

## Team Roster
| Agent | Type | Work Item | Status | Conv ID |
|-------|------|-----------|--------|---------|
| Explorer 1 (Failed) | teamwork_preview_explorer | Scan kernel (arch, boot, drivers, ecs, ipc, lib, net, sys) | failed | a9b46288-5a4e-47b9-9d4a-e35a5bb8af56 |
| Explorer 1 (Failed 2) | teamwork_preview_explorer | Scan kernel (arch, boot, drivers, ecs, ipc, lib, net, sys) | failed | 5b976e68-1ace-4a5e-9275-11b8ae8c1e14 |
| Explorer 1 (Repl 2) | teamwork_preview_explorer | Scan kernel (arch, boot, drivers, ecs, ipc, lib, net, sys) | pending | 98718a75-0802-4039-be17-2ef1eb9ceaca |
| Explorer 2 (Failed) | teamwork_preview_explorer | Scan user, keuos_rt, lattice_ecs, benchmarks | failed | 306ec49a-535c-4f01-8728-433c0140edd6 |
| Explorer 2 (Repl) | teamwork_preview_explorer | Scan user, keuos_rt, lattice_ecs, benchmarks | pending | e49ad66e-81f0-438f-aa9a-5ac4b74fc7ad |
| Explorer 3 | teamwork_preview_explorer | Scan tools, docs, top-level files | pending | 0068ebac-33e3-4ad8-96ec-5f23d1f42ffc |

## Succession Status
- Succession required: no
- Spawn count: 6 / 16
- Pending subagents: 98718a75-0802-4039-be17-2ef1eb9ceaca, e49ad66e-81f0-438f-aa9a-5ac4b74fc7ad, 0068ebac-33e3-4ad8-96ec-5f23d1f42ffc
- Predecessor: none
- Successor: not yet spawned

## Active Timers
- Heartbeat cron: not started
- Safety timer: none
- On succession: kill all timers before spawning successor
- On context truncation: run `manage_task(Action="list")` — re-create if missing

## Artifact Index
- /Users/kevin/projects/lattice/.agents/sub_orch_keuos/ORIGINAL_REQUEST.md — Original User Request
- /Users/kevin/projects/lattice/.agents/sub_orch_keuos/BRIEFING.md — Persistent memory index
- /Users/kevin/projects/lattice/.agents/sub_orch_keuos/progress.md — Liveness and checkpoint file
- /Users/kevin/projects/lattice/.agents/sub_orch_keuos/SCOPE.md — Milestone M2_keuos decomposition and status
