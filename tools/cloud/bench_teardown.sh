#!/bin/bash
# =============================================================================
# Teardown persistent benchmark instance.
# =============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
source "$ROOT/tools/cloud/cloud_config.sh"

GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

STATE_FILE="$ROOT/.bench_instance"

if [ ! -f "$STATE_FILE" ]; then
    echo -e "${RED}No instance running.${NC}"
    exit 0
fi
source "$STATE_FILE"

echo "Terminating $INSTANCE_ID..."
aws ec2 terminate-instances --instance-ids "$INSTANCE_ID" \
    --region "$AWS_REGION" --output text > /dev/null 2>&1 || true

# Cancel any spot requests associated with this instance
aws ec2 describe-spot-instance-requests --region "$AWS_REGION" \
    --filters "Name=state,Values=open,active" \
    --query 'SpotInstanceRequests[*].SpotInstanceRequestId' --output text 2>/dev/null \
    | xargs -r aws ec2 cancel-spot-instance-requests \
    --region "$AWS_REGION" --spot-instance-request-ids 2>/dev/null || true

rm -f "$STATE_FILE"
echo -e "${GREEN}✓ Instance terminated, state cleaned up${NC}"
