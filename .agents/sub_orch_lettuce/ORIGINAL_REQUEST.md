# Original User Request

## Initial Request — 2026-07-11T15:11:08-07:00

You are the Sub-Orchestrator for the Lettuce repository audit (Milestone M4_lettuce).
Working directory: /Users/kevin/projects/lattice/.agents/sub_orch_lettuce
Repository to audit: /Users/kevin/projects/lettuce

Your mission is to perform a comprehensive audit of all proprietary source code and documentation in /Users/kevin/projects/lettuce to eradicate AI slop, hyperbole, and legacy artifacts, while ensuring the code remains correct and tests pass.

Workflow Protocol:
1. Initialize your BRIEFING.md and progress.md in your working directory (/Users/kevin/projects/lattice/.agents/sub_orch_lettuce). Set up a recurring heartbeat cron (every 10 minutes) and safety timer.
2. Initialize your SCOPE.md. Record M4_lettuce milestone details and status.
3. Execute the iteration loop per the Project pattern:
   - Spawn 3 Explorer(s) in subdirectories (under your working directory) to scan the codebase for slop/hyperbole/legacy artifacts and propose clean-ups. Exclude vendor dependencies, test fixtures, and generated files.
   - Spawn a Worker in a subdirectory to implement the clean-ups, run builds/tests (make test), and verify.
     MANDATORY INTEGRITY WARNING to Worker: "DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A Forensic Auditor will independently verify your work."
   - Spawn 2 Reviewer(s) to verify the correctness, completeness, and check constraints (no mutants).
   - Spawn 2 Challenger(s) to verify correctness.
   - Spawn a Forensic Auditor (teamwork_preview_auditor) to perform integrity verification. If auditor reports INTEGRITY VIOLATION, fail iteration immediately and loop back with auditor's full evidence.
4. When verification passes:
   - Commit and push the resulting fixes directly to the `main` branch of `/Users/kevin/projects/lettuce`. Do NOT add any Co-Authored-By or Signed-off-by trailers attributing work to Claude/Anthropic.
   - Log all proprietary files scanned in your handoff.
5. Create a handoff.md in your working directory detailing:
   - Verification command outputs
   - Scan log of audited files
   - Git commit hash of pushed changes
6. Send a completion message to parent orchestrator (conversation ID: c6ca45a3-f366-44f9-a89d-17fc7dc03fef) with the path to your handoff.md.
