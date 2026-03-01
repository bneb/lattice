#!/usr/bin/env python3
"""
Lattice / Salt — Stochastic Financial Exit Model
=================================================

Monte Carlo simulation of possible financial exit scenarios for the
Lattice unikernel + Salt language project, informed by real-world
comparable transactions in the Hard Tech / systems infrastructure space.

Comparable Transactions (2016–2026):
  - Unikernel Systems → Docker (2016)           ~$10M  (estimated, undisclosed)
  - Tilera → EZchip (2014)                      $50–130M (custom silicon)
  - Inflection AI → Microsoft (2024)            $650M  (pseudo-acqui-hire)
  - Adept → Amazon (2024)                       ~$400M (talent + license, est.)
  - Character.ai → Google (2024)                $2.7B  (talent + license)
  - Windsurf → Google (2025)                    $2.4B  (talent + license)
  - HashiCorp → IBM (2025)                      $6.4B  (infra software, full acq.)
  - Solo.io valuation (2021 Series C)           $1B+   (networking infra)

Markets: 75th-percentile AI acquisition deal size tripled from $82M
(2020) to $248M (2025).  Source: tomtunguz.com analysis of Big Tech M&A.

Usage:
    python3 scripts/financial_exit_model.py
"""

import random
import math
import statistics
import sys

# ---------------------------------------------------------------------------
# Seed for reproducibility
# ---------------------------------------------------------------------------
random.seed(2026_03_01)

N_SIMULATIONS = 100_000

# ═══════════════════════════════════════════════════════════════════════════
# 1. EXIT SCENARIO DEFINITIONS
# ═══════════════════════════════════════════════════════════════════════════
# Each scenario is: (name, probability_weight, value_sampler)
#
# value_sampler() returns a single draw in $M.
# We model each scenario's value as a log-normal distribution fitted to
# comparable transactions.


def lognormal_draw(median_m: float, sigma: float) -> float:
    """Draw from a log-normal distribution parameterised by median ($M) and
    shape sigma (log-space std dev).  median = exp(mu), so mu = ln(median)."""
    mu = math.log(median_m)
    return random.lognormvariate(mu, sigma)


# --- Scenario samplers ---------------------------------------------------

def sample_shutdown() -> float:
    """Total loss — project abandoned or team disperses."""
    return 0.0


def sample_acquihire_small() -> float:
    """Small acqui-hire by mid-tier infra company (e.g., Fastly, Fly.io,
    Vercel).  Comparable: Unikernel Systems → Docker (~$10M est.)"""
    return lognormal_draw(median_m=12.0, sigma=0.5)


def sample_acquihire_large() -> float:
    """Major acqui-hire by Tier-1 cloud (AWS, GCP, Azure, Cloudflare).
    Comparable: Inflection→MSFT ($650M), Adept→Amazon (~$400M).
    We use a lower median because Lattice is pre-revenue."""
    return lognormal_draw(median_m=80.0, sigma=0.7)


def sample_ip_license() -> float:
    """IP / technology licensing deal — cloud provider licenses the Z3
    verification engine + SPSC IPC stack without full acquisition.
    Comparable: pseudo-acqui-hire licensing structures (2024 trend)."""
    return lognormal_draw(median_m=25.0, sigma=0.6)


def sample_full_acquisition() -> float:
    """Full strategic acquisition by a hyperscaler building edge/unikernel
    products.  Comparable: HashiCorp→IBM ($6.4B at maturity). Lattice is
    far earlier stage, so median is dramatically lower."""
    return lognormal_draw(median_m=200.0, sigma=0.8)


def sample_series_a_then_exit() -> float:
    """Team raises Series A ($5–15M), builds for 2–3 years, then exits.
    Applies a dilution / time-discount factor."""
    series_a = lognormal_draw(median_m=10.0, sigma=0.4)
    exit_multiple = lognormal_draw(median_m=6.0, sigma=0.6)
    dilution_factor = random.uniform(0.25, 0.50)  # founders retain 25-50%
    return series_a * exit_multiple * dilution_factor


def sample_open_source_ecosystem() -> float:
    """Project becomes a successful open-source foundation (à la Linux
    Foundation, CNCF).  Indirect value via consulting / support contracts.
    Modest direct financial exit."""
    return lognormal_draw(median_m=5.0, sigma=0.8)


# ═══════════════════════════════════════════════════════════════════════════
# 2. SCENARIO PROBABILITY WEIGHTS
# ═══════════════════════════════════════════════════════════════════════════
# These are calibrated against the base rates from the memorandum's
# Bayesian model (60% acqui-hire posterior) and general Hard Tech exit data.

SCENARIOS = [
    # (name,                     weight,  sampler)
    ("Shutdown / No Exit",        0.15,   sample_shutdown),
    ("Small Acqui-hire",          0.25,   sample_acquihire_small),
    ("Large Acqui-hire (Tier-1)", 0.20,   sample_acquihire_large),
    ("IP / Technology License",   0.10,   sample_ip_license),
    ("Full Strategic Acquisition",0.08,   sample_full_acquisition),
    ("Series A → Delayed Exit",   0.15,   sample_series_a_then_exit),
    ("Open-Source Ecosystem",     0.07,   sample_open_source_ecosystem),
]

# Normalize weights
total_weight = sum(w for _, w, _ in SCENARIOS)
SCENARIOS = [(n, w / total_weight, s) for n, w, s in SCENARIOS]

# ═══════════════════════════════════════════════════════════════════════════
# 3. BUYER PROFILE DEFINITIONS
# ═══════════════════════════════════════════════════════════════════════════

BUYERS = {
    "AWS / Amazon":     {"affinity": 0.25, "premium_mult": 1.2},
    "Cloudflare":       {"affinity": 0.20, "premium_mult": 1.3},
    "Google / GCP":     {"affinity": 0.15, "premium_mult": 1.1},
    "Microsoft / Azure":{"affinity": 0.10, "premium_mult": 1.0},
    "Fastly":           {"affinity": 0.08, "premium_mult": 0.7},
    "Fly.io":           {"affinity": 0.07, "premium_mult": 0.6},
    "IBM":              {"affinity": 0.05, "premium_mult": 0.9},
    "Other / Unknown":  {"affinity": 0.10, "premium_mult": 0.8},
}

def pick_buyer() -> tuple[str, float]:
    """Pick a buyer weighted by affinity, return (name, premium_multiplier)."""
    names = list(BUYERS.keys())
    weights = [BUYERS[n]["affinity"] for n in names]
    chosen = random.choices(names, weights=weights, k=1)[0]
    return chosen, BUYERS[chosen]["premium_mult"]


# ═══════════════════════════════════════════════════════════════════════════
# 4. MONTE CARLO ENGINE
# ═══════════════════════════════════════════════════════════════════════════

def run_simulation(n: int = N_SIMULATIONS):
    names   = [s[0] for s in SCENARIOS]
    weights = [s[1] for s in SCENARIOS]
    samplers = [s[2] for s in SCENARIOS]

    results = []           # (scenario_name, buyer_name, exit_value_$M)
    scenario_counts = {n: 0 for n in names}
    buyer_counts = {b: 0 for b in BUYERS}

    for _ in range(n):
        # Pick scenario
        idx = random.choices(range(len(SCENARIOS)), weights=weights, k=1)[0]
        scenario_name = names[idx]
        base_value = samplers[idx]()

        # Pick buyer (only for non-shutdown scenarios)
        if base_value > 0:
            buyer_name, premium = pick_buyer()
            exit_value = base_value * premium
        else:
            buyer_name = "N/A"
            exit_value = 0.0

        results.append((scenario_name, buyer_name, exit_value))
        scenario_counts[scenario_name] += 1
        if buyer_name != "N/A":
            buyer_counts[buyer_name] += 1

    return results, scenario_counts, buyer_counts


# ═══════════════════════════════════════════════════════════════════════════
# 5. ANALYTICS & REPORTING
# ═══════════════════════════════════════════════════════════════════════════

def percentile(data: list[float], p: float) -> float:
    """Simple percentile (nearest-rank)."""
    if not data:
        return 0.0
    k = int(math.ceil(p / 100.0 * len(data))) - 1
    return sorted(data)[max(0, k)]


def print_report(results, scenario_counts, buyer_counts):
    all_values = [v for _, _, v in results]
    nonzero    = [v for v in all_values if v > 0]

    print("=" * 72)
    print("  LATTICE / SALT — STOCHASTIC FINANCIAL EXIT MODEL")
    print(f"  {N_SIMULATIONS:,} Monte Carlo simulations")
    print("=" * 72)

    # --- Overall Statistics ------------------------------------------------
    print("\n── OVERALL EXIT VALUE DISTRIBUTION ($M) ──\n")
    print(f"  Mean (all outcomes):      ${statistics.mean(all_values):>10.1f}M")
    print(f"  Mean (conditional, >$0):  ${statistics.mean(nonzero):>10.1f}M")
    print(f"  Median:                   ${statistics.median(all_values):>10.1f}M")
    print(f"  Std Dev:                  ${statistics.stdev(all_values):>10.1f}M")
    print(f"  P10:                      ${percentile(all_values, 10):>10.1f}M")
    print(f"  P25:                      ${percentile(all_values, 25):>10.1f}M")
    print(f"  P50:                      ${percentile(all_values, 50):>10.1f}M")
    print(f"  P75:                      ${percentile(all_values, 75):>10.1f}M")
    print(f"  P90:                      ${percentile(all_values, 90):>10.1f}M")
    print(f"  P99:                      ${percentile(all_values, 99):>10.1f}M")
    print(f"  Max:                      ${max(all_values):>10.1f}M")

    # --- Expected Value by Scenario ----------------------------------------
    print("\n── EXPECTED VALUE BY SCENARIO ──\n")
    print(f"  {'Scenario':<30s} {'Freq':>6s}  {'Mean $M':>9s}  {'P50 $M':>8s}  {'P90 $M':>8s}")
    print(f"  {'─' * 30} {'─' * 6}  {'─' * 9}  {'─' * 8}  {'─' * 8}")

    for name in scenario_counts:
        vals = [v for sn, _, v in results if sn == name]
        freq_pct = len(vals) / len(results) * 100
        if vals:
            m = statistics.mean(vals)
            p50 = statistics.median(vals)
            p90 = percentile(vals, 90)
        else:
            m = p50 = p90 = 0.0
        print(f"  {name:<30s} {freq_pct:>5.1f}%  ${m:>8.1f}  ${p50:>7.1f}  ${p90:>7.1f}")

    # --- Buyer Frequency ---------------------------------------------------
    print("\n── BUYER FREQUENCY (non-shutdown exits) ──\n")
    total_exits = sum(buyer_counts.values())
    print(f"  {'Buyer':<25s} {'Count':>7s}  {'Share':>6s}")
    print(f"  {'─' * 25} {'─' * 7}  {'─' * 6}")
    for buyer, count in sorted(buyer_counts.items(), key=lambda x: -x[1]):
        share = count / total_exits * 100 if total_exits > 0 else 0
        print(f"  {buyer:<25s} {count:>7,d}  {share:>5.1f}%")

    # --- Risk Metrics ------------------------------------------------------
    print("\n── RISK METRICS ──\n")
    zero_pct = sum(1 for v in all_values if v == 0) / len(all_values) * 100
    below_10 = sum(1 for v in all_values if 0 < v < 10) / len(all_values) * 100
    above_100 = sum(1 for v in all_values if v >= 100) / len(all_values) * 100
    above_500 = sum(1 for v in all_values if v >= 500) / len(all_values) * 100
    print(f"  P(total loss):            {zero_pct:>6.1f}%")
    print(f"  P(exit < $10M):           {below_10:>6.1f}%")
    print(f"  P(exit ≥ $100M):          {above_100:>6.1f}%")
    print(f"  P(exit ≥ $500M):          {above_500:>6.1f}%")

    # --- Histogram (text-based) --------------------------------------------
    print("\n── EXIT VALUE HISTOGRAM ($M, log scale buckets) ──\n")
    buckets = [0, 1, 5, 10, 25, 50, 100, 250, 500, 1000, float('inf')]
    bucket_labels = [
        "     $0 (shutdown)",
        "   $0–1M",
        "   $1–5M",
        "  $5–10M",
        " $10–25M",
        " $25–50M",
        "$50–100M",
        "$100–250M",
        "$250–500M",
        "   $500M+",
    ]
    bucket_counts = [0] * len(bucket_labels)

    for v in all_values:
        if v == 0:
            bucket_counts[0] += 1
        else:
            for i in range(1, len(buckets)):
                if buckets[i - 1] < v <= buckets[i]:
                    bucket_counts[i - 1 if i <= 1 else i - 1] += 1
                    break

    # recount properly
    bucket_counts = [0] * 10
    for v in all_values:
        if v == 0:
            bucket_counts[0] += 1
        elif v <= 1:
            bucket_counts[1] += 1
        elif v <= 5:
            bucket_counts[2] += 1
        elif v <= 10:
            bucket_counts[3] += 1
        elif v <= 25:
            bucket_counts[4] += 1
        elif v <= 50:
            bucket_counts[5] += 1
        elif v <= 100:
            bucket_counts[6] += 1
        elif v <= 250:
            bucket_counts[7] += 1
        elif v <= 500:
            bucket_counts[8] += 1
        else:
            bucket_counts[9] += 1

    max_count = max(bucket_counts) if bucket_counts else 1
    bar_width = 40
    for label, count in zip(bucket_labels, bucket_counts):
        bar_len = int(count / max_count * bar_width)
        pct = count / len(all_values) * 100
        bar = "█" * bar_len
        print(f"  {label}  {bar:<{bar_width}s} {count:>6,d} ({pct:>5.1f}%)")

    print("\n" + "=" * 72)
    print("  END OF REPORT")
    print("=" * 72)


# ═══════════════════════════════════════════════════════════════════════════
# 6. MAIN
# ═══════════════════════════════════════════════════════════════════════════

if __name__ == "__main__":
    print("Running Monte Carlo simulation...\n")
    results, scenario_counts, buyer_counts = run_simulation()
    print_report(results, scenario_counts, buyer_counts)
