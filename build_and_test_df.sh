#!/bin/bash
set -e
cd /Users/kevin/projects/lattice

export DYLD_LIBRARY_PATH=/opt/homebrew/lib
SALT_FRONT=salt-front/target/release/salt-front
SALT_OPT=salt/build/salt-opt
LLC=/opt/homebrew/opt/llvm/bin/llc
LLD=/Users/kevin/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/rust-lld
SED_FIX='s/"target-cpu"="[^"]*"/"target-cpu"="x86-64"/g; s/"target-features"="[^"]*"/"target-features"=""/g; s/getelementptr inbounds nuw/getelementptr inbounds/g'

compile_salt() {
    local src=$1 out=$2
    $SALT_FRONT "$src" --lib --no-verify --disable-alias-scopes > "/tmp/${out}.mlir"
    $SALT_OPT --emit-llvm --verify=false < "/tmp/${out}.mlir" 2>/dev/null | sed "$SED_FIX" > "/tmp/${out}.ll"
    $LLC "/tmp/${out}.ll" -filetype=obj -o "qemu_build/${out}.o" \
        -relocation-model=pic -mtriple=x86_64-none-elf -mcpu=x86-64
}

echo "=== Building Double Fault Panic Target ==="

compile_salt kernel/core/df_test_runner.salt df_test_runner

mv qemu_build/suite.o qemu_build/suite.o.bak || true

$LLD -flavor gnu -T kernel/arch/x86/linker.ld -o qemu_build/kernel_df.elf \
    -z max-page-size=0x1000 qemu_build/*.o

mv qemu_build/suite.o.bak qemu_build/suite.o || true

echo "=== Running Double Fault Panic Test ==="
python3 run_panic_test.py qemu_build/kernel_df.elf
