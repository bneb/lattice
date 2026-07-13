## 2026-07-11T22:11:21Z

You are explorer_facet_2.
Your working directory is: /Users/kevin/projects/lattice/.agents/explorer_facet_2
Your parent is sub_orch_facet (conversation ID: 9b44c990-7377-405f-bcbd-1257dd8b81dd).

Your task:
1. Initialize your BRIEFING.md and progress.md in your working directory (overwrite if they exist).
2. Read the project scope from /Users/kevin/projects/lattice/.agents/sub_orch_facet/SCOPE.md.
3. Perform a read-only scan of the following directories in /Users/kevin/projects/facet:
   - raster/
   - compositor/
   Verify and scan for:
   - AI slop (generated comments, boilerplate sentences like "Certainly!", formatting/commenting that looks artificial, etc.)
   - Hyperbole (marketing claims, exaggeration)
   - Legacy artifacts (unused commented-out code, temp files, TODO/FIXME/HACK/XXX/temp_/workaround annotations that violate check-constraints)
   Exclude vendor dependencies, test fixtures, and generated files.
4. Prepare a structured handoff report in your working directory detailing:
   - List of audited files.
   - Specific findings of AI slop, hyperbole, and legacy artifacts with file paths and line ranges.
   - Recommended cleanup strategy.
5. Send a completion message to parent sub_orch_facet (conversation ID: 9b44c990-7377-405f-bcbd-1257dd8b81dd) with the path to your handoff report.
