#!/bin/bash
# Block Co-Authored-By trailers from ever entering the repo
if git log --format='%B' -1 | grep -q "Co-Authored-By\|Signed-off-by.*noreply@anthropic"; then
  echo "BLOCKED: commit message contains Co-Authored-By or Anthropic attribution trailer." >&2
  echo "These are permanently banned. Remove the trailer and recommit." >&2
  exit 1
fi
