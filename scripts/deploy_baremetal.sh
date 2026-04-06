#!/usr/bin/env zsh
# =============================================================================
# deploy_baremetal.sh — Deploy KeuOS Exokernel to Bare Metal (UM890 Pro)
# =============================================================================
#
# Transfers the compiled kernel.elf to the target machine, installs a GRUB2
# menu entry, and optionally reboots into KeuOS.
#
# Usage:
#   ./scripts/deploy_baremetal.sh --host 192.168.1.x
#   ./scripts/deploy_baremetal.sh --host um890.local --user kevin
#   ./scripts/deploy_baremetal.sh --host um890.local --user kevin --reboot
#   ./scripts/deploy_baremetal.sh --host um890.local --rebuild   # rebuild ELF first
#
# Prerequisites (on Mac):
#   - SSH key-based auth to the UM890 (run: ssh-copy-id user@host)
#   - sudo NOPASSWD for /usr/sbin/update-grub on UM890 (or enter password)
#
# Prerequisites (on UM890):
#   - Ubuntu with GRUB2 (standard install)
#   - /boot/keuos/ directory will be created by this script
#
# =============================================================================

set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"

# ── Defaults ──────────────────────────────────────────────────────────────────
HOST=""
USER="ubuntu"
REBOOT=false
REBUILD=false
KERNEL_ELF="$PROJECT_ROOT/qemu_build/kernel.elf"
REMOTE_BOOT_DIR="/boot/keuos"
GRUB_CUSTOM="/etc/grub.d/40_custom"

# ── Arg parsing ───────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)    HOST="$2";    shift 2 ;;
        --user)    USER="$2";    shift 2 ;;
        --elf)     KERNEL_ELF="$2"; shift 2 ;;
        --reboot)  REBOOT=true;  shift ;;
        --rebuild) REBUILD=true; shift ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

if [[ -z "$HOST" ]]; then
    echo "❌  --host is required"
    echo "    Usage: $0 --host <ip-or-hostname> [--user ubuntu] [--reboot] [--rebuild]"
    exit 1
fi

SSH_TARGET="$USER@$HOST"
SSH_OPTS=(-o StrictHostKeyChecking=accept-new -o ConnectTimeout=10)

echo "╔══════════════════════════════════════════════════════╗"
echo "║   KeuOS → Bare Metal Deploy                        ║"
echo "║   Target: $SSH_TARGET"
echo "║   Kernel: $KERNEL_ELF"
echo "╚══════════════════════════════════════════════════════╝"
echo ""

# ── Optional: rebuild ELF from source (macOS path) ───────────────────────────
if $REBUILD; then
    echo "🔨 Rebuilding kernel.elf..."
    "$SCRIPT_DIR/run_qemu.sh" --build-only
    echo ""
fi

# ── Verify ELF exists ─────────────────────────────────────────────────────────
if [[ ! -f "$KERNEL_ELF" ]]; then
    echo "❌  Kernel ELF not found: $KERNEL_ELF"
    echo "    Run with --rebuild, or build with: ./scripts/run_qemu.sh --build-only"
    exit 1
fi

ELF_SIZE=$(wc -c < "$KERNEL_ELF" | tr -d ' ')
echo "  [✓] Kernel ELF: $KERNEL_ELF (${ELF_SIZE} bytes)"

# ── Verify SSH connectivity ───────────────────────────────────────────────────
echo "  Checking SSH connectivity to $SSH_TARGET..."
if ! ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "echo ok" &>/dev/null; then
    echo "❌  Cannot reach $SSH_TARGET via SSH."
    echo "    Ensure the machine is online and you have key-based auth:"
    echo "    ssh-copy-id $SSH_TARGET"
    exit 1
fi
echo "  [✓] SSH connection OK"

# ── Create remote /boot/keuos/ directory ────────────────────────────────────
echo ""
echo "📁 Preparing remote /boot/keuos/ ..."
ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "sudo mkdir -p $REMOTE_BOOT_DIR && sudo chown $USER:$USER $REMOTE_BOOT_DIR"

# ── rsync kernel ELF ──────────────────────────────────────────────────────────
echo "🚀 Transferring kernel.elf..."
rsync -avz --progress \
    -e "ssh ${SSH_OPTS[*]}" \
    "$KERNEL_ELF" \
    "$SSH_TARGET:$REMOTE_BOOT_DIR/kernel.elf"
echo "  [✓] Kernel transferred to $REMOTE_BOOT_DIR/kernel.elf"

# ── Transfer GRUB stanza ──────────────────────────────────────────────────────
echo ""
echo "🔧 Installing GRUB menu entry..."

# Detect the boot partition that GRUB uses (usually the root or /boot partition)
GRUB_ROOT=$(ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "
    # Find the device hosting /boot
    BOOT_DEV=\$(df /boot 2>/dev/null | awk 'NR==2 {print \$1}')
    if [[ -z \"\$BOOT_DEV\" ]]; then
        BOOT_DEV=\$(df / | awk 'NR==2 {print \$1}')
    fi

    # Convert /dev/sdaX or /dev/nvme0n1pX → GRUB (hdN,gptM) or (hdN,msdosM) notation
    DISK=\$(echo \"\$BOOT_DEV\" | sed 's/p\?[0-9]*$//')
    PART=\$(echo \"\$BOOT_DEV\" | grep -oE '[0-9]+$')
    DISK_IDX=0  # Assume first disk; can be overridden below

    # Map disk letter (sda→0, sdb→1, nvme0n1→0 etc.)
    DISK_LETTER=\$(echo \"\$DISK\" | grep -oE '[a-z]+[0-9]*$' | head -1)
    case \"\$DISK_LETTER\" in
        sda|nvme0n1) DISK_IDX=0 ;;
        sdb|nvme1n1) DISK_IDX=1 ;;
        sdc)         DISK_IDX=2 ;;
    esac

    # Check partition table type
    PTTYPE=\$(sudo blkid -o value -s PTTYPE \"\$DISK\" 2>/dev/null || echo gpt)
    if [[ \"\$PTTYPE\" == \"gpt\" ]]; then
        echo \"hd\${DISK_IDX},gpt\${PART}\"
    else
        echo \"hd\${DISK_IDX},msdos\${PART}\"
    fi
")

echo "  GRUB root detected: ($GRUB_ROOT)"

# Determine if /boot is a separate partition (affects path in GRUB)
BOOT_IS_SEPARATE=$(ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "
    df /boot | awk 'NR==2 {print \$1}' | xargs -I{} df / | awk 'NR==2 {exit (\$1 == \"{}\")}' && echo yes || echo no
" 2>/dev/null || echo "no")

if [[ "$BOOT_IS_SEPARATE" == "yes" ]]; then
    GRUB_KERNEL_PATH="/keuos/kernel.elf"
else
    GRUB_KERNEL_PATH="/boot/keuos/kernel.elf"
fi

# Write GRUB stanza remotely
ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "
cat <<'GRUB_EOF' | sudo tee -a $GRUB_CUSTOM > /dev/null

# ---- KeuOS Exokernel (bare metal) ----
menuentry 'KeuOS Exokernel (Multiboot1)' --class os {
    insmod part_gpt
    insmod ext2
    set root='($GRUB_ROOT)'
    echo 'Loading KeuOS Exokernel...'
    multiboot $GRUB_KERNEL_PATH
    boot
}
GRUB_EOF
echo '[✓] GRUB stanza appended to $GRUB_CUSTOM'
sudo update-grub 2>&1 | grep -E '(Found|Generating|done|keuos)' || true
echo '[✓] GRUB updated'
"

echo ""
echo "════════════════════════════════════════════════════════"
echo "  [✓] Deploy complete!"
echo "  To verify: ssh ${SSH_OPTS[*]} $SSH_TARGET 'grep -A6 KeuOS /boot/grub/grub.cfg'"
echo ""

# ── Optional: capture the next boot serial output via socat ──────────────────
echo "  Serial output tip:"
echo "    On UM890:  sudo socat /dev/ttyS0,raw,b115200 stdout"
echo "    (or plug monitor — kernel serial is COM1 / 0x3F8 at 115200 baud)"
echo ""

# ── Optional: reboot ──────────────────────────────────────────────────────────
if $REBOOT; then
    echo "🔁 Rebooting $SSH_TARGET into KeuOS..."
    echo "   (Select 'KeuOS Exokernel' in GRUB if not set as default)"
    ssh $SSH_OPTS "$SSH_TARGET" "sudo reboot" || true
    echo ""
    echo "   Machine is rebooting. Monitor serial output at COM1 (115200 8N1)."
else
    echo "  To reboot now: ssh $SSH_TARGET 'sudo reboot'"
    echo "  Select 'KeuOS Exokernel (Multiboot1)' in the GRUB menu."
fi
