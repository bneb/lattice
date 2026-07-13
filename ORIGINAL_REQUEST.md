# Original User Request

## Initial Request — 2026-07-11T22:09:15Z

Audit every file across the newly disaggregated Salt ecosystem repositories (salt, keuos, basalt, lettuce, facet). Ensure high code quality and rigorously eliminate any "AI slop," hyperbole, or embarrassing legacy artifacts before the repositories are considered finalized.

Working directory: /Users/kevin/projects/
Integrity mode: development

## Requirements

### R1. Cross-Repository Audit
Audit all proprietary source code and documentation across the `salt`, `keuos`, `basalt`, `lettuce`, and `facet` repositories. Exclude vendor dependencies, test fixtures, and generated files.

### R2. Eradicate Slop
Identify and actively rewrite any code comments, documentation, or structures that contain "AI slop" (e.g., sycophantic language, unnecessary filler, overly verbose explanations, confabulations), hyperbole, or embarrassing legacy artifacts. Replace them with terse, pristine, and staff-engineer-quality technical writing. 

### R3. Commit and Push
Directly commit and push the resulting fixes to the `main` branch of each respective repository. Ensure commit messages adhere strictly to the user's rules (e.g., no AI attribution trailers).

## Acceptance Criteria

### Audit Completeness
- [ ] The team outputs a definitive log proving that proprietary files in all 5 repositories were scanned.

### Independent Verification
- [ ] An independent agent acting as a judge reviews the final Git diffs for all 5 repositories and certifies that the changes strictly improve quality without introducing new "slop."

### Regression Prevention
- [ ] All test suites (e.g., `cargo test` in `salt-front`, `sp check` where applicable) continue to pass perfectly after the rewrites are pushed.
