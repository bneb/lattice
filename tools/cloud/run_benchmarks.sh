#!/bin/bash
# ============================================================================
# Lattice Cloud Benchmark Runner
# ============================================================================
# Launches an AWS c5.metal spot instance, runs the Lattice kernel benchmark
# suite with KVM acceleration, captures results, and terminates the instance.
#
# Usage:
#   ./tools/cloud/run_benchmarks.sh              # Full run
#   ./tools/cloud/run_benchmarks.sh --dry-run     # Print commands without executing
#   ./tools/cloud/run_benchmarks.sh --skip-setup   # Skip toolchain install (re-run)
#
# Prerequisites:
#   - AWS CLI configured (brew install awscli && aws configure)
#   - EC2 key pair and security group (see cloud_config.sh)
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Load configuration
source "$SCRIPT_DIR/cloud_config.sh"

# Parse arguments
DRY_RUN=false
SKIP_SETUP=false
for arg in "$@"; do
    case $arg in
        --dry-run) DRY_RUN=true ;;
        --skip-setup) SKIP_SETUP=true ;;
        *) echo "Unknown argument: $arg"; exit 1 ;;
    esac
done

# Colors
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

INSTANCE_ID=""
INSTANCE_IP=""

# ─── Cleanup on exit ──────────────────────────────────────────────
cleanup() {
    if [ -n "$INSTANCE_ID" ]; then
        echo ""
        echo -e "${YELLOW}Terminating instance ${INSTANCE_ID}...${NC}"
        aws ec2 terminate-instances \
            --instance-ids "$INSTANCE_ID" \
            --region "$AWS_REGION" \
            --output text > /dev/null 2>&1 || true
        echo -e "${GREEN}✓ Instance terminated${NC}"
    fi
}
trap cleanup EXIT

# ─── Helpers ───────────────────────────────────────────────────────
run_or_print() {
    if $DRY_RUN; then
        echo -e "${CYAN}[DRY RUN]${NC} $*"
        return 0
    fi
    "$@"
}

ssh_cmd() {
    ssh -i "$EC2_KEY_PATH" \
        -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR \
        -o ConnectTimeout=10 \
        "${EC2_USER}@${INSTANCE_IP}" "$@"
}

echo ""
echo -e "${CYAN}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║${NC}  ${BOLD}Lattice Kernel Benchmarks${NC} — AWS c5.metal / KVM        ${CYAN}║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════╝${NC}"
echo ""

# ─── Step 0: Validate prerequisites ───────────────────────────────
echo -e "${YELLOW}[0/7]${NC} Validating prerequisites..."

if ! command -v aws &>/dev/null; then
    echo -e "${RED}  ✗ AWS CLI not found. Install: brew install awscli${NC}"
    exit 1
fi

if ! aws sts get-caller-identity --region "$AWS_REGION" &>/dev/null; then
    echo -e "${RED}  ✗ AWS CLI not configured. Run: aws configure${NC}"
    exit 1
fi

if [ ! -f "$EC2_KEY_PATH" ]; then
    echo -e "${RED}  ✗ SSH key not found at ${EC2_KEY_PATH}${NC}"
    echo "  Create one: aws ec2 create-key-pair --key-name ${EC2_KEY_NAME} --region ${AWS_REGION} --query 'KeyMaterial' --output text > ${EC2_KEY_PATH} && chmod 400 ${EC2_KEY_PATH}"
    exit 1
fi

echo -e "${GREEN}  ✓ AWS CLI configured, SSH key found${NC}"

# ─── Step 1: Resolve security group ID ─────────────────────────────
echo -e "${YELLOW}[1/7]${NC} Resolving security group..."
SG_ID=$(aws ec2 describe-security-groups \
    --group-names "$EC2_SECURITY_GROUP" \
    --region "$AWS_REGION" \
    --query 'SecurityGroups[0].GroupId' \
    --output text 2>/dev/null || echo "")

if [ -z "$SG_ID" ] || [ "$SG_ID" = "None" ]; then
    echo -e "${YELLOW}  Creating security group '${EC2_SECURITY_GROUP}'...${NC}"
    SG_ID=$(aws ec2 create-security-group \
        --group-name "$EC2_SECURITY_GROUP" \
        --description "Lattice benchmark SSH access" \
        --region "$AWS_REGION" \
        --query 'GroupId' \
        --output text)

    MY_IP=$(curl -s --max-time 5 ifconfig.me || echo "0.0.0.0")
    aws ec2 authorize-security-group-ingress \
        --group-id "$SG_ID" \
        --protocol tcp \
        --port 22 \
        --cidr "${MY_IP}/32" \
        --region "$AWS_REGION" > /dev/null
    echo -e "${GREEN}  ✓ Created security group ${SG_ID} (SSH from ${MY_IP})${NC}"
else
    echo -e "${GREEN}  ✓ Security group: ${SG_ID}${NC}"
fi

# ─── Step 2: Launch spot instance ──────────────────────────────────
echo -e "${YELLOW}[2/7]${NC} Launching ${EC2_INSTANCE_TYPE} spot instance..."

if $DRY_RUN; then
    echo -e "${CYAN}[DRY RUN]${NC} Would launch ${EC2_INSTANCE_TYPE} in ${AWS_REGION}"
    echo -e "${CYAN}[DRY RUN]${NC} AMI: ${EC2_AMI}, Key: ${EC2_KEY_NAME}"
    INSTANCE_ID="i-dry-run-00000000"
    INSTANCE_IP="1.2.3.4"
else
    # Try spot first (cheaper), fall back to on-demand if quota exceeded
    INSTANCE_ID=$(aws ec2 run-instances \
        --image-id "$EC2_AMI" \
        --instance-type "$EC2_INSTANCE_TYPE" \
        --key-name "$EC2_KEY_NAME" \
        --security-group-ids "$SG_ID" \
        --region "$AWS_REGION" \
        --instance-market-options '{"MarketType":"spot","SpotOptions":{"MaxPrice":"'"$EC2_MAX_SPOT_PRICE"'","SpotInstanceType":"one-time"}}' \
        --block-device-mappings '[{"DeviceName":"/dev/sda1","Ebs":{"VolumeSize":30,"VolumeType":"gp3"}}]' \
        --tag-specifications 'ResourceType=instance,Tags=[{Key=Name,Value=lattice-benchmark}]' \
        --query 'Instances[0].InstanceId' \
        --output text 2>/dev/null || echo "SPOT_FAILED")

    if [ "$INSTANCE_ID" = "SPOT_FAILED" ]; then
        echo -e "${YELLOW}  Spot request failed (quota or capacity) — launching on-demand...${NC}"
        INSTANCE_ID=$(aws ec2 run-instances \
            --image-id "$EC2_AMI" \
            --instance-type "$EC2_INSTANCE_TYPE" \
            --key-name "$EC2_KEY_NAME" \
            --security-group-ids "$SG_ID" \
            --region "$AWS_REGION" \
            --block-device-mappings '[{"DeviceName":"/dev/sda1","Ebs":{"VolumeSize":30,"VolumeType":"gp3"}}]' \
            --tag-specifications 'ResourceType=instance,Tags=[{Key=Name,Value=lattice-benchmark}]' \
            --query 'Instances[0].InstanceId' \
            --output text)
        echo -e "${GREEN}  ✓ On-demand instance launched: ${INSTANCE_ID} (\$4.08/hr)${NC}"
    else
        echo -e "${GREEN}  ✓ Spot instance launched: ${INSTANCE_ID}${NC}"
    fi
fi

# ─── Step 3: Wait for instance to be running ───────────────────────
echo -e "${YELLOW}[3/7]${NC} Waiting for instance to be running..."

if ! $DRY_RUN; then
    aws ec2 wait instance-running \
        --instance-ids "$INSTANCE_ID" \
        --region "$AWS_REGION"

    INSTANCE_IP=$(aws ec2 describe-instances \
        --instance-ids "$INSTANCE_ID" \
        --region "$AWS_REGION" \
        --query 'Reservations[0].Instances[0].PublicIpAddress' \
        --output text)

    echo -e "${GREEN}  ✓ Instance running at ${INSTANCE_IP}${NC}"

    # Wait for SSH to become available
    echo -e "  Waiting for SSH..."
    for i in $(seq 1 60); do
        if ssh_cmd "echo ok" &>/dev/null; then
            break
        fi
        sleep 5
        printf "."
    done
    echo ""
    echo -e "${GREEN}  ✓ SSH connected${NC}"
fi

# ─── Step 4: Sync repo ────────────────────────────────────────────
echo -e "${YELLOW}[4/7]${NC} Syncing repository..."

if $DRY_RUN; then
    echo -e "${CYAN}[DRY RUN]${NC} Would rsync ${ROOT}/ to ${EC2_USER}@1.2.3.4:lattice/"
else
    rsync -az --delete \
        --exclude '.git' \
        --exclude 'target' \
        --exclude 'salt/build' \
        --exclude 'node_modules' \
        --exclude '.DS_Store' \
        --exclude 'qemu_build' \
        --exclude '*.log' \
        --exclude '*.profraw' \
        --exclude 'coverage_report' \
        --exclude '.bench_basalt' \
        -e "ssh -i $EC2_KEY_PATH -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR" \
        "$ROOT/" \
        "${EC2_USER}@${INSTANCE_IP}:lattice/"

    echo -e "${GREEN}  ✓ Repository synced${NC}"
fi

# ─── Step 5: Setup toolchain ──────────────────────────────────────
if ! $SKIP_SETUP; then
    echo -e "${YELLOW}[5/7]${NC} Setting up toolchain (first run takes ~3-5 min)..."

    if $DRY_RUN; then
        echo -e "${CYAN}[DRY RUN]${NC} Would run setup_instance.sh on remote"
    else
        ssh_cmd "bash lattice/tools/cloud/setup_instance.sh" 2>&1 | \
            while IFS= read -r line; do
                echo "  $line"
            done
        echo -e "${GREEN}  ✓ Toolchain ready${NC}"
    fi
else
    echo -e "${GREEN}[5/7]${NC} Skipping setup (--skip-setup)"
fi

# ─── Step 6: Run benchmarks ───────────────────────────────────────
echo -e "${YELLOW}[6/7]${NC} Running benchmark suite with KVM..."
echo ""
echo -e "${CYAN}────────────────── BENCHMARK OUTPUT ──────────────────${NC}"
echo ""

if $DRY_RUN; then
    echo -e "${CYAN}[DRY RUN]${NC} Would run: python3 tools/runner_qemu.py run"
    BENCH_OUTPUT="[dry run — no output]"
else
    BENCH_OUTPUT=$(ssh_cmd "cd lattice && python3 tools/runner_qemu.py bench" 2>&1 || true)
    echo "$BENCH_OUTPUT"
fi

echo ""
echo -e "${CYAN}────────────────────────────────────────────────────────${NC}"
echo ""

# ─── Step 7: Parse and display results ─────────────────────────────
echo -e "${YELLOW}[7/7]${NC} Results summary:"
echo ""

if ! $DRY_RUN; then
    # Extract benchmark lines
    echo "$BENCH_OUTPUT" | grep -E "BENCH:|ROF Result|BENCHMARK SUITE" | while IFS= read -r line; do
        echo -e "  ${GREEN}${line}${NC}"
    done

    # Check for KVM confirmation
    if echo "$BENCH_OUTPUT" | grep -qi "kvm\|accel"; then
        echo ""
        echo -e "  ${GREEN}✓ KVM acceleration confirmed${NC}"
    fi

    # Check for success
    if echo "$BENCH_OUTPUT" | grep -q "BENCHMARK SUITE COMPLETE"; then
        echo ""
        echo -e "${GREEN}${BOLD}✓ Benchmark suite completed successfully.${NC}"
        echo -e "  Platform: AWS ${EC2_INSTANCE_TYPE} (Intel Xeon Platinum 8275CL)"
        echo -e "  Acceleration: QEMU/KVM (hardware virtualization)"
    else
        echo ""
        echo -e "${YELLOW}⚠ Benchmark may not have completed fully. Review output above.${NC}"
    fi

    # Save results to file
    RESULTS_FILE="$ROOT/docs/cloud_benchmark_results.txt"
    echo "$BENCH_OUTPUT" > "$RESULTS_FILE"
    echo ""
    echo -e "  Raw output saved to: ${BOLD}docs/cloud_benchmark_results.txt${NC}"
fi

echo ""
echo -e "Instance ${INSTANCE_ID} will be terminated on exit."
echo -e "To keep it running, press Ctrl+C now and manually terminate later:"
echo -e "  aws ec2 terminate-instances --instance-ids ${INSTANCE_ID} --region ${AWS_REGION}"
echo ""
