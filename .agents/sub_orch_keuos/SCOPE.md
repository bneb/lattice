# Scope: KeuOS Repository Audit (M2_keuos)

## Architecture
- Repository: `/Users/kevin/projects/keuos`
- Codebase includes: kernel source (excluding `kernel/core/`, `kernel/mem/`, `kernel/sched/`), userspace programs (`user/`), tools (`tools/`), documentation (`README.md`, etc.), tests (`tests/`, `kernel/tests/`).
- Subagent outputs located under: `/Users/kevin/projects/lattice/.agents/sub_orch_keuos/`

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Scan & Propose | Spawn Explorers to scan the codebase for slop, hyperbole, and legacy artifacts and propose clean-ups. | none | IN_PROGRESS |
| 2 | Clean & Build | Spawn Worker to implement proposed clean-ups and verify that `make kernel` and `python3 tools/run_all_tests.py` pass. | M1 | PLANNED |
| 3 | Review & Verify | Spawn Reviewers and Challengers to verify correctness, completeness, check constraints, and performance. | M2 | PLANNED |
| 4 | Forensic Audit | Spawn Forensic Auditor to perform integrity validation and confirm there is no cheating or hardcoding. | M3 | PLANNED |
| 5 | Commit & Push | Commit clean-ups to `main` branch without Claude attribution trailers and submit final handoff report. | M4 | PLANNED |

## Interface Contracts
- Cleaned files must compile cleanly with `make kernel`.
- Cleaned files must pass `python3 tools/run_all_tests.py` in the `/Users/kevin/projects/keuos` repository.
- Changes must strictly obey the `<32 LOC/fn`, `<500 LOC/file`, and `<3 nesting` constraints, as well as no TODO/FIXME/HACK/XXX/temp_/workaround mutants in non-test source.
