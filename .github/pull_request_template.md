## Description
<!-- What does this PR do? -->

## Motivation
<!-- Why is this change needed? Link related issues. -->

Closes #

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation
- [ ] Performance improvement
- [ ] Refactor (no behavior change)

## Testing
<!-- How was this tested? -->
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` clean
- [ ] Kernel TDD gates pass (`tools/runner_qemu.py` reports GREEN)
- [ ] Added new tests for new behavior

## Checklist
- [ ] I have read CONTRIBUTING.md
- [ ] Architecture-agnostic code uses the HAL router (not arch-specific imports)
- [ ] Public API changes are reflected in docs/
- [ ] ABI changes are reflected in docs/abi/KEUOS_ABI.md
- [ ] Z3 contracts are present on new unsafe operations
- [ ] No hardcoded absolute paths
- [ ] No TODO/FIXME/HACK markers (opened an issue instead)
