#!/bin/bash
# ============================================================================
# Cloud Benchmark Configuration
# ============================================================================
# Edit these values before your first run.
# ============================================================================

# AWS Region — us-east-1 has the widest c5.metal spot availability
AWS_REGION="us-east-1"

# EC2 Key Pair — name of your SSH key pair (created via AWS Console or CLI)
# Create one:  aws ec2 create-key-pair --key-name lattice-bench --query 'KeyMaterial' --output text > ~/.ssh/lattice-bench.pem && chmod 400 ~/.ssh/lattice-bench.pem
EC2_KEY_NAME="lattice-bench"
EC2_KEY_PATH="$HOME/.ssh/lattice-bench.pem"

# Security Group — must allow SSH (port 22) from your IP
# Create one:  aws ec2 create-security-group --group-name lattice-bench-sg --description "Lattice benchmark SSH access"
#              aws ec2 authorize-security-group-ingress --group-name lattice-bench-sg --protocol tcp --port 22 --cidr $(curl -s ifconfig.me)/32
EC2_SECURITY_GROUP="lattice-bench-sg"

# Instance type — z1d.metal for bare-metal x86_64 with KVM (48 vCPUs, fits default quota)
# c5.metal (96 vCPUs) requires a quota increase; z1d.metal works out of the box
EC2_INSTANCE_TYPE="z1d.metal"

# AMI — Ubuntu 24.04 LTS x86_64 (us-east-1). Update if using a different region.
# Find AMIs: aws ec2 describe-images --owners 099720109477 --filters "Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*" --query 'Images | sort_by(@, &CreationDate) | [-1].ImageId' --output text
EC2_AMI="ami-0136735c2bb5cf5bf"

# SSH user for Ubuntu AMIs
EC2_USER="ubuntu"

# Max spot price ($/hr) — set slightly above typical spot price to avoid interruption
# On-demand c5.metal is $4.08/hr, spot is typically ~$1.40-1.80/hr
EC2_MAX_SPOT_PRICE="2.50"

# Timeout for benchmark run (seconds)
BENCHMARK_TIMEOUT=600
