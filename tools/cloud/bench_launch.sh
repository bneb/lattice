#!/bin/bash
# =============================================================================
# Launch a persistent z1d.metal instance for benchmark iteration.
# Saves instance ID and IP to .bench_instance for use by bench_run.sh.
# =============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
source "$ROOT/tools/cloud/cloud_config.sh"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m'

STATE_FILE="$ROOT/.bench_instance"

if [ -f "$STATE_FILE" ]; then
    echo -e "${YELLOW}Instance already running. Use bench_run.sh or bench_teardown.sh${NC}"
    cat "$STATE_FILE"
    exit 0
fi

echo -e "${CYAN}Launching $EC2_INSTANCE_TYPE (on-demand)...${NC}"
INSTANCE_ID=$(aws ec2 run-instances \
    --region "$AWS_REGION" \
    --image-id "$EC2_AMI" \
    --instance-type "$EC2_INSTANCE_TYPE" \
    --key-name "$EC2_KEY_NAME" \
    --security-groups "$EC2_SECURITY_GROUP" \
    --query 'Instances[0].InstanceId' \
    --output text)
echo "  ✓ Instance: $INSTANCE_ID"

echo "Waiting for instance..."
aws ec2 wait instance-running --instance-ids "$INSTANCE_ID" --region "$AWS_REGION"
INSTANCE_IP=$(aws ec2 describe-instances \
    --instance-ids "$INSTANCE_ID" --region "$AWS_REGION" \
    --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
echo "  ✓ Running at $INSTANCE_IP"

SSH_OPTS="-i $EC2_KEY_PATH -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

echo -n "Waiting for SSH..."
for i in $(seq 1 60); do
    if ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" "true" 2>/dev/null; then
        echo ""
        echo "  ✓ SSH connected"
        break
    fi
    echo -n "."
    sleep 5
done

echo "Installing QEMU..."
ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" \
    "sudo apt-get update -qq && sudo apt-get install -y -qq qemu-system-x86 > /dev/null 2>&1 && sudo chmod 666 /dev/kvm"
echo "  ✓ QEMU + KVM ready"

# Save state
cat > "$STATE_FILE" << EOF
INSTANCE_ID=$INSTANCE_ID
INSTANCE_IP=$INSTANCE_IP
EOF

echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  Instance ready for benchmarking!${NC}"
echo -e "${GREEN}  IP: $INSTANCE_IP${NC}"
echo -e "${GREEN}  Run:      ./tools/cloud/bench_run.sh${NC}"
echo -e "${GREEN}  Teardown: ./tools/cloud/bench_teardown.sh${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════${NC}"
