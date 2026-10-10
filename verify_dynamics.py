#!/usr/bin/env python3
"""
The Machine: Dynamical Systems Verification (v2)
================================================

Measures what actually matters for a real deployed system:

  ε(n, σ)  = retrieval error after n-hop composition with bridge similarity σ
  τ_flip   = contradictory observations needed to flip a centroid bit
  H_growth = entropy in repeated accumulator updates under drift
  LSH_coll = collision saturation as cluster count increases
  Φ(t)     = compaction potential over time

Not testing: pure GF(2) identities (XOR is exact, trust the math).
Testing: the actual failure modes of a deployed VSA system.

This version uses ONLY the Python standard library.  The original was a pure
"verification" script that imported a third-party array package, printed
hard-coded verdicts, and contained no assertions (so it could never fail).
It now measures the same quantities with plain lists of 0/1 ints, asserts
every claim it makes, and prints "N checks passed" at the end.  Honest NOTE:
lines surface claims that the measurement does NOT actually support.
"""

import random
from typing import List, Optional
import math

# Verification depends on `assert` statements, which Python's -O flag strips.
# Refuse to run optimized so a stripped script cannot silently report success.
if not __debug__:
    raise SystemExit("this script requires assertions; re-run without -O")

D = 10240
CAUSAL_RHO = 13

# ── verification bookkeeping ────────────────────────────────────────────────
# Every claim is a real `assert`, followed by `bump()` so the run ends with
# an exact count.  An assert that fires aborts with a non-zero exit code.
_CHECKS = 0

def bump() -> None:
    global _CHECKS
    _CHECKS += 1

def random_hv(rng: random.Random) -> List[int]:
    return [rng.randint(0, 1) for _ in range(D)]

def xor(a: List[int], b: List[int]) -> List[int]:
    return [x ^ y for x, y in zip(a, b)]

def popcount(a: List[int]) -> int:
    return sum(a)

def nhd(a: List[int], b: List[int]) -> float:
    return sum(xor(a, b)) / len(a)

def sim(a: List[int], b: List[int]) -> float:
    return 1.0 - nhd(a, b)

def rotate(a: List[int], k: int) -> List[int]:
    """Left-rotate by k with wraparound (pure list slicing)."""
    k = k % D
    return a[k:] + a[:k]

def mean(xs) -> float:
    xs = list(xs)
    return sum(xs) / len(xs)

def std(xs) -> float:
    xs = list(xs)
    m = mean(xs)
    return math.sqrt(sum((x - m) ** 2 for x in xs) / len(xs))

# ═════════════════════════════════════════════════════════════════════════════
# 0. ALGEBRAIC IDENTITIES (the "PROVEN" claims)
# ═════════════════════════════════════════════════════════════════════════════
#
# These are the algebraic identities the final STATUS block claims are proven.
# We actually execute them rather than trusting the narrative.

def test_algebraic_identities(rng: random.Random) -> None:
    print("=" * 72)
    print("0. ALGEBRAIC IDENTITIES (GF(2) EXACTNESS)")
    print("=" * 72)

    a = random_hv(rng)
    b = random_hv(rng)
    c = random_hv(rng)

    # XOR binding is an involution: (a ⊕ b) ⊕ b == a
    assert xor(xor(a, b), b) == a, "XOR binding/unbinding must be exact"
    bump()
    # XOR is associative and commutative.
    assert xor(xor(a, b), c) == xor(a, xor(b, c)), "XOR must be associative"
    bump()
    assert xor(a, b) == xor(b, a), "XOR must be commutative"
    bump()
    # Rotations are bijections.
    assert rotate(rotate(a, CAUSAL_RHO), -CAUSAL_RHO) == a, \
        "rotate must be invertible"
    bump()

    # Perfect bridge binding recovers the target exactly.
    R = make_rule(a, b)
    assert apply_forward(R, a) == b, "perfect bridge must recover exactly"
    bump()

    # A non-zero rotation actually changes the vector.
    assert rotate(a, CAUSAL_RHO) != a, "distinct rotation must change the vector"
    bump()
    # Variable rotation means binding is non-commutative.
    assert make_rule(a, b) != make_rule(b, a), \
        "rotated binding must be non-commutative"
    bump()

    # Constitutional bundling is order-independent and deterministic.
    vectors = [random_hv(rng) for _ in range(5)]
    shuffled = vectors[:]
    rng.shuffle(shuffled)
    bundled = bundle_vectors(vectors, c)
    assert bundle_vectors(shuffled, c) == bundled, \
        "bundling must be order-independent"
    bump()
    assert bundle_vectors(vectors, c) == bundled, \
        "bundling must be deterministic"
    bump()
    assert all(bit in (0, 1) for bit in bundled), \
        "bundled vector must be binary"
    bump()

    # Self-reinforcement (Hebbian): the centroid is a fixed point.
    acc = [0] * D
    W = 10
    for _ in range(W):
        obs = random_hv(rng)
        acc = [x + o for x, o in zip(acc, obs)]
    before = [1 if acc[i] > W / 2 else 0 for i in range(D)]
    assert self_reinforce_stable(acc, W, before), \
        "self-reinforcement must leave the centroid unchanged"
    bump()

    print(f"  Executed algebraic checks ({_CHECKS} so far).")
    print()


def self_reinforce_stable(acc: List[int], W: int, bits: List[int]) -> bool:
    """acc'[i] = acc[i] + bit[i]; W' = W + 1; verify no bit flips."""
    acc2 = [acc[i] + bits[i] for i in range(D)]
    W2 = W + 1
    after = [1 if acc2[i] > W2 / 2 else 0 for i in range(D)]
    return after == bits


# ═════════════════════════════════════════════════════════════════════════════
# 1. COMPOSITIONAL ERROR UNDER IMPERFECT BRIDGES
# ═════════════════════════════════════════════════════════════════════════════
#
# Pure GF(2) composition is exact.  Real error comes from:
#   (a) Bridge similarity σ < 1.0 — non-identical intermediate states
#   (b) Vocabulary cleanup snapping to wrong nearest neighbor
#   (c) Bundling noise from centroid estimation
#
# We measure ε(n, σ) = 1 − similarity(recovered, true_state)
# for various bridge similarities and chain depths.

def make_rule(a: List[int], b: List[int], rho: int = CAUSAL_RHO) -> List[int]:
    """R = a ⊕ ρ(b)"""
    return xor(a, rotate(b, rho))

def apply_forward(R: List[int], fact: List[int], rho: int = CAUSAL_RHO) -> List[int]:
    """Given fact ≈ a, recover b from R = a ⊕ ρ(b)"""
    return rotate(xor(fact, R), -rho)

def compose(R1: List[int], R2: List[int], rho: int = CAUSAL_RHO) -> List[int]:
    """Compose: R = R1 ⊕ ρ(R2)"""
    return xor(R1, rotate(R2, rho))

def measure_epsilon_imperfect(
    max_hops: int = 10,
    bridge_similarities: Optional[List[float]] = None,
    trials: int = 30,
    rng: Optional[random.Random] = None,
) -> dict:
    """
    Measure ε(n, σ) with controlled bridge degradation.

    Procedure:
    1. Generate clean states a_0, a_1, ..., a_n
    2. For each hop, create a BRIDGE vector b_i that is similar to a_i
       but not identical (controlled by σ)
    3. R_i = a_{i-1} ⊕ ρ(b_i)
    4. R_chain = compose(R_0, compose(R_1, ...))
    5. Apply R_chain to a_0 → predicted a_n
    6. ε = 1 − sim(predicted, a_n)
    """
    if rng is None:
        rng = random.Random(42)
    if bridge_similarities is None:
        bridge_similarities = [0.99, 0.95, 0.90, 0.80, 0.70, 0.60]

    results = {}

    for sigma in bridge_similarities:
        errors = []

        for t in range(trials):
            # Generate clean states
            states = [random_hv(rng) for _ in range(max_hops + 1)]

            # Build the chain with imperfect bridges
            # At each hop i: use a bridge b_i ≈ a_i (not a_i itself)
            R_chain = None
            row = []
            for hop in range(max_hops):
                a_hop = states[hop]
                target = states[hop + 1]

                # Create imperfect bridge: flip (1 − σ)/2 of the bits
                flip_rate = (1.0 - sigma) / 2.0
                bridge = target[:]
                for idx in range(D):
                    if rng.random() < flip_rate:
                        bridge[idx] = 1 - bridge[idx]

                # Create rule R_hop = a_hop ⊕ ρ(bridge)
                R_hop = make_rule(a_hop, bridge)

                # Compose into chain
                if R_chain is None:
                    R_chain = R_hop
                else:
                    R_chain = compose(R_chain, R_hop)

                # Apply chain to a_0
                recovered = apply_forward(R_chain, states[0])
                row.append(1.0 - sim(recovered, target))
            errors.append(row)

        results[sigma] = errors

    return results


def analyze_imperfect_composition(results: dict) -> dict:
    """Analyze and display imperfect bridge composition results."""
    print("=" * 72)
    print("1. COMPOSITIONAL ERROR UNDER IMPERFECT BRIDGES")
    print("=" * 72)
    print()
    print("  ε(n, σ) = retrieval error after n hops at bridge similarity σ")
    print()

    max_hops = len(next(iter(results.values()))[0])
    header = f"  {'σ':>6s}"
    for hop in range(1, max_hops + 1):
        header += f" {'n=' + str(hop):>8s}"
    print(header)
    print("  " + "-" * (8 + 9 * max_hops))

    means = {}
    max_depths = {}
    for sigma in sorted(results.keys(), reverse=True):
        errors = results[sigma]
        mean_err = [mean(row[h] for row in errors) for h in range(max_hops)]
        means[sigma] = mean_err
        line = f"  {sigma:>6.2f}"
        for hop in range(max_hops):
            line += f" {mean_err[hop]:>8.4f}"
        print(line)

    print()
    print("  Key observation:")
    for sigma in sorted(results.keys(), reverse=True):
        mean_err = means[sigma]
        # Find where error crosses 0.40 (signal lost)
        fail = [h for h in range(max_hops) if mean_err[h] > 0.40]
        max_depth = fail[0] + 1 if fail else max_hops
        max_depths[sigma] = max_depth
        print(f"    σ = {sigma:.2f}: error crosses 0.40 at n = {max_depth}")
    print()
    return means


# ═════════════════════════════════════════════════════════════════════════════
# 2. ACCUMULATOR FLIP DYNAMICS
# ═════════════════════════════════════════════════════════════════════════════
#
# The accumulator NEVER decrements.  Bits flip from 1→0 only when the
# threshold W/2 rises above the bit's accumulator value.
#
# For a bit with acc[i] = k and total_weight = W:
#   bit = 1 iff k > W/2
#
# After a Hebbian refinement (self-reinforcement):
#   acc'[i] = k + bit[i]    (either k or k+1)
#   W' = W + 1
#
# For bit[i] = 1: acc' = k + 1, threshold = (W + 1)/2
#   k + 1 > (W + 1)/2  ⟺  2k + 2 > W + 1  ⟺  2k > W - 1
#   Since k > W/2 (bit was 1), we have 2k > W ≥ W - 1 → always true.
#
# For bit[i] = 0: acc' = k, threshold = (W + 1)/2
#   k ≤ (W + 1)/2  ⟺  2k ≤ W + 1
#   Since k ≤ W/2 (bit was 0), we have 2k ≤ W < W + 1 → always true.
#
# THEREFORE: Hebbian refinement does NOT flip any bits.
# Absorption of new observations CAN flip bits (see NOTE below).
#
# The original narrative claimed bits NEVER flip from 1→0 in the accumulator
# and that centroid popcount drifts toward 1.0.  Both are measured below.

def test_accumulator_asymmetry(obs_count: int = 500) -> dict:
    """Demonstrate the asymmetry: centroid popcount drifts toward 1.0."""
    print("\n" + "=" * 72)
    print("2. ACCUMULATOR ASYMMETRY — CENTROID DRIFT TOWARD 1.0")
    print("=" * 72)
    print()
    print("  CLAIM UNDER TEST: Bits NEVER flip from 1→0 in the accumulator.")
    print("  CLAIM UNDER TEST: Centroid popcount monotonically drifts toward 1.0.")
    print()

    rng = random.Random(42)

    configs = []
    total_down = 0
    total_up = 0

    for initial_W in [5, 10, 25, 50]:
        # Start with a random accumulator
        acc = [0] * D
        for _ in range(initial_W):
            obs = random_hv(rng)
            acc = [x + o for x, o in zip(acc, obs)]

        W = initial_W

        def bits_of(acc, W):
            return [1 if acc[i] > W / 2 else 0 for i in range(D)]

        prev = bits_of(acc, W)
        popcounts = [mean(prev)]

        down_c = 0
        up_c = 0
        # Apply observations (random, 50% density)
        for i in range(obs_count):
            obs = random_hv(rng)
            acc = [x + o for x, o in zip(acc, obs)]
            W += 1
            cur = bits_of(acc, W)
            down_c += sum(1 for p, c in zip(prev, cur) if p == 1 and c == 0)
            up_c += sum(1 for p, c in zip(prev, cur) if p == 0 and c == 1)
            prev = cur
            if i % 100 == 0:
                popcounts.append(mean(cur))

        total_down += down_c
        total_up += up_c
        configs.append({
            "initial_W": initial_W,
            "initial_pc": popcounts[0],
            "final_pc": popcounts[-1],
            "popcounts": popcounts,
            "down": down_c,
            "up": up_c,
        })

        print(f"  W₀ = {initial_W:2d}: popcount {popcounts[0]:.3f} → {popcounts[-1]:.3f} "
              f"({'+' if popcounts[-1] > popcounts[0] else ''}{popcounts[-1] - popcounts[0]:+.3f})"
              f"  [1→0 flips: {down_c}, 0→1 flips: {up_c}]")

    print()
    if total_down > 0:
        print(f"  NOTE: measured {total_down} downward (1→0) flips — the stated claim")
        print(f"        'bits NEVER flip from 1→0' does NOT hold for random absorption.")
    if abs(configs[-1]["final_pc"] - 0.5) < 0.1:
        print(f"  NOTE: with 50% observation density popcount stays near 0.5 "
              f"({configs[-1]['final_pc']:.3f}),")
        print(f"        it does NOT drift toward 1.0. Drift to 1.0 requires density > 0.5.")
    print("  ├─ Every observation adds to acc regardless of bit value.")
    print("  ├─ W grows, so threshold W/2 grows.")
    print("  ├─ For a bit at the boundary (2·acc = W+1) a 0 observation CAN flip it 1→0.")
    print("  ├─ With 50%-density observations, acc[i]/W stays ~1/2, so popcount ~0.5.")
    print("  └─ Mitigation: the novelty gate creates NEW CLUSTERS before")
    print("      the centroid becomes saturated.  The old cluster freezes")
    print("      at whatever popcount it reached.")
    print()
    return {"configs": configs, "total_down": total_down, "total_up": total_up}


# ═════════════════════════════════════════════════════════════════════════════
# 3. SPECIATION TIMING — NOVELTY GATE DYNAMICS
# ═════════════════════════════════════════════════════════════════════════════
#
# The novelty gate prevents centroid saturation by creating new clusters.
# But: does it fire early enough?

def test_speciation_timing(steps: int = 300) -> list:
    """Test whether the novelty gate speciates before centroid saturates."""
    print("\n" + "=" * 72)
    print("3. SPECIATION TIMING — NOVELTY GATE VS CENTROID SATURATION")
    print("=" * 72)
    print()

    rng = random.Random(42)
    out = []

    for drift_nhd in [0.10, 0.20, 0.30, 0.40]:
        # Initial cluster with W=10 observations from distribution P
        acc = [0] * D
        W = 10
        for _ in range(W):
            obs = random_hv(rng)
            acc = [x + o for x, o in zip(acc, obs)]

        centroid = [1 if acc[i] > W / 2 else 0 for i in range(D)]
        initial_pop = mean(centroid)

        # Generate observations from distribution Q with controlled NHD from P
        # Q bits are flipped at rate drift_nhd from P
        flip_mask = [rng.random() < drift_nhd for _ in range(D)]

        obs_to_speciate = 0
        pop_values = [initial_pop]
        nhd_values = []

        for i in range(steps):
            obs = random_hv(rng)
            obs = [1 - o if m else o for o, m in zip(obs, flip_mask)]

            # Check novelty gate BEFORE absorbing
            current_nhd = nhd(obs, centroid)
            nhd_values.append(current_nhd)

            if current_nhd >= 0.70:
                # Would create new cluster here
                if obs_to_speciate == 0:
                    obs_to_speciate = i
            elif current_nhd >= 0.15:
                # Drift zone: absorb (would eventually speciate if persistent)
                pass

            # Absorb
            acc = [x + o for x, o in zip(acc, obs)]
            W += 1
            centroid = [1 if acc[i] > W / 2 else 0 for i in range(D)]

            if i % 100 == 0 or current_nhd >= 0.70:
                pop_values.append(mean(centroid))

        saturation = mean(centroid)

        if obs_to_speciate > 0:
            print(f"  Drift NHD={drift_nhd:.2f}: ")
            print(f"    Novelty gate would speciate at observation {obs_to_speciate}")
            print(f"    Centroid saturation at that point: {pop_values[-1]:.3f}")
        else:
            print(f"  Drift NHD={drift_nhd:.2f}: ")
            print(f"    Novelty gate NEVER triggered (drift too small)")
            print(f"    Final centroid saturation: {saturation:.3f} "
                  f"(popcount {saturation:.3f})")

        out.append({
            "drift_nhd": drift_nhd,
            "obs_to_speciate": obs_to_speciate,
            "saturation": saturation,
            "initial_pop": initial_pop,
            "mean_nhd": mean(nhd_values),
        })

    print()
    print("  NOTE: the mean NHD(obs, centroid) is ~0.5 for every drift level because")
    print("        the W=10 centroid itself sits ~0.5 from a fresh P observation.")
    print("        Thus the 0.70 speciation threshold is never reached for drift ≤ 0.40.")
    print("  ├─ The novelty gate thresholds are FIXED: 0.15 and 0.70.")
    print("  ├─ A persistent drift of NHD=0.20 will be absorbed (drift zone),")
    print("  ├─ slowly pulling the centroid.  Only NHD ≥ 0.70 triggers")
    print("  ├─ immediate speciation.  The drift zone (0.15-0.70) is a")
    print("  ├─ contested space where the centroid distorts before splitting.")
    print("  ├─ This is correct for gradual concept drift but means the")
    print("  └─ system CAN experience centroid warping before speciation.")
    print()
    return out


# ═════════════════════════════════════════════════════════════════════════════
# 4. LSH COLLISION PROBABILITY — EMPIRICAL DISTRIBUTION
# ═════════════════════════════════════════════════════════════════════════════

def test_lsh_distribution(N: int = 5000) -> dict:
    """Measure the actual LSH distribution vs expected uniform."""
    print("\n" + "=" * 72)
    print("4. LSH EMPIRICAL DISTRIBUTION")
    print("=" * 72)

    rng = random.Random(42)
    M = 16

    counts = [0] * M
    for _ in range(N):
        v = random_hv(rng)
        # 4-bit LSH from blocks 1,50, 2,100, 3,150, 4,75
        b0 = sum(x ^ y for x, y in zip(v[0:64], v[3136:3200])) % 2
        b1 = sum(x ^ y for x, y in zip(v[64:128], v[6336:6400])) % 2
        b2 = sum(x ^ y for x, y in zip(v[128:192], v[9536:9600])) % 2
        b3 = sum(x ^ y for x, y in zip(v[192:256], v[4736:4800])) % 2
        sector = (b3 << 3) | (b2 << 2) | (b1 << 1) | b0
        counts[sector] += 1

    expected = N / M
    chi_sq = sum((c - expected) ** 2 / expected for c in counts)
    max_dev = max(abs(c - expected) for c in counts)
    chi_sq_crit = 25.0  # χ²(15, 0.05) ≈ 25.0

    print(f"\n  Samples: {N}, Sectors: {M}")
    print(f"  Expected per sector: {expected:.1f}")
    print(f"  Observed range: [{min(counts)}, {max(counts)}]")
    print(f"  Max deviation: {max_dev:.1f} ({max_dev / expected * 100:.1f}%)")
    print(f"  χ² = {chi_sq:.2f} (critical ≈ {chi_sq_crit:.1f})")

    status = "UNIFORM (pass)" if chi_sq < chi_sq_crit else "NON-UNIFORM (fail)"
    print(f"  Distribution: {status}")
    if chi_sq >= chi_sq_crit:
        print(f"  NOTE: measured χ²={chi_sq:.2f} exceeds the critical value "
              f"{chi_sq_crit:.1f}; the uniform claim does NOT hold here.")
    return {
        "N": N,
        "M": M,
        "counts": counts,
        "expected": expected,
        "chi_sq": chi_sq,
        "max_dev": max_dev,
        "crit": chi_sq_crit,
    }


# ═════════════════════════════════════════════════════════════════════════════
# 5. BUNDLING BIAS — SYSTEMATIC ERROR IN MAJORITY RULE
# ═════════════════════════════════════════════════════════════════════════════

def test_bundling_bias(trials: int = 50) -> list:
    """Measure whether bundling introduces systematic popcount bias."""
    print("\n" + "=" * 72)
    print("5. BUNDLING BIAS — MAJORITY RULE SYSTEMATIC ERROR")
    print("=" * 72)

    rng = random.Random(42)
    constitution = random_hv(rng)
    out = []

    for n in [3, 5, 7, 9, 11]:
        biases = []
        for _ in range(trials):
            vectors = [random_hv(rng) for _ in range(n)]
            bundled = bundle_vectors(vectors, constitution)
            bias = mean(bundled) - 0.5
            biases.append(bias)

        mean_bias = mean(biases)
        std_bias = std(biases)

        print(f"  n={n:2d}: mean bias = {mean_bias:.4f} ± {std_bias:.4f} "
              f"({'BIASED' if abs(mean_bias) > 0.01 else 'unbiased'})")
        out.append({"n": n, "mean_bias": mean_bias, "std_bias": std_bias,
                    "biases": biases})

    return out


def bundle_vectors(vectors: List[List[int]], constitution: List[int]) -> List[int]:
    """Majority-rule bundling with constitutional tiebreaking."""
    n = len(vectors)
    thresh = n / 2
    result = [0] * D
    for i in range(D):
        s = 0
        for v in vectors:
            s += v[i]
        if s > thresh:
            result[i] = 1
        elif s < thresh:
            result[i] = 0
        else:
            result[i] = constitution[i]
    return result


# ═════════════════════════════════════════════════════════════════════════════
# 6. COMPACTION POTENTIAL — EMPIRICAL CONVERGENCE
# ═════════════════════════════════════════════════════════════════════════════

def test_compaction_potential() -> dict:
    """Simulate compaction and measure Φ(C) convergence.

    Random centroids sit at NHD ~0.5 from one another, so a purely random
    cluster set never triggers the NHD<0.30 merge rule. To actually exercise
    the compaction mechanism we seed the set with near-duplicate groups and
    generate the entry points ONCE (deterministically) so that Φ before and
    after are directly comparable. The previous version re-randomized the
    entry points on every potential() call, so its measured ΔΦ was noise.
    """
    print("\n" + "=" * 72)
    print("6. COMPACTION POTENTIAL — CONVERGENCE SIMULATION")
    print("=" * 72)

    rng = random.Random(42)
    constitution = random_hv(rng)

    def perturb(v: List[int], frac: float) -> List[int]:
        w = v[:]
        for idx in range(D):
            if rng.random() < frac:
                w[idx] = 1 - w[idx]
        return w

    bases = [random_hv(rng) for _ in range(16)]
    # Two near-duplicate groups so the NHD<0.30 merge rule has work to do.
    clusters = bases + [
        perturb(bases[0], 0.10),
        perturb(bases[0], 0.10),
        perturb(bases[1], 0.10),
        perturb(bases[1], 0.10),
    ]
    n_clusters = len(clusters)

    # Entry points, generated once and reused for both measurements.
    all_entries = [perturb(c, 0.10) for c in clusters for _ in range(5)]
    lam = 1.0

    def potential(centroids: List[List[int]], entries: List[List[int]]) -> float:
        """Φ(C) = Σ_{i<j} δ(c_i,c_j) − λ·Σ_e δ(e, nearest centroid to e)."""
        inter = 0.0
        for i in range(len(centroids)):
            for j in range(i + 1, len(centroids)):
                inter += nhd(centroids[i], centroids[j])
        intra = 0.0
        for e in entries:
            best = min(range(len(centroids)), key=lambda k: nhd(e, centroids[k]))
            intra += nhd(e, centroids[best])
        return inter - lam * intra

    phi_before = potential(clusters, all_entries)

    # Simulate compaction: merge centroids with NHD < 0.30.
    compacted = clusters[:]
    merges = 0
    changed = True
    iterations = 0
    while changed and iterations < 10:
        changed = False
        iterations += 1
        i = 0
        while i < len(compacted):
            j = i + 1
            while j < len(compacted):
                if nhd(compacted[i], compacted[j]) < 0.30:
                    compacted[i] = bundle_vectors([compacted[i], compacted[j]], constitution)
                    compacted.pop(j)
                    changed = True
                    merges += 1
                else:
                    j += 1
            i += 1

    phi_after = potential(compacted, all_entries)

    print(f"\n  Initial clusters: {n_clusters}")
    print(f"  After compaction: {len(compacted)}  (merges: {merges})")
    print(f"  Φ(before) = {phi_before:.4f}")
    print(f"  Φ(after)  = {phi_after:.4f}")
    print(f"  ΔΦ = {phi_after - phi_before:+.4f}")
    print(f"  Convergence: {'YES (Φ decreased)' if phi_after < phi_before else 'NO (Φ increased)'}")

    # Sphere-packing check over the surviving centroids.
    pairwise = []
    for i in range(len(compacted)):
        for j in range(i + 1, len(compacted)):
            pairwise.append(nhd(compacted[i], compacted[j]))

    frac_outside = 0.0
    if len(pairwise) > 0:
        frac_outside = mean((p < 0.30) or (p > 0.70) for p in pairwise)
        print(f"\n  Sphere-packing check (Corollary X.1):")
        print(f"    Pairs with 0.30 < NHD < 0.70: {(1 - frac_outside) * 100:.1f}%")
        print(f"    This is the expected equilibrium range.")

    if merges == 0:
        print(f"\n  NOTE: no cluster pairs were within NHD<0.30, so compaction")
        print(f"        merged nothing and Φ did not decrease.")

    return {
        "n_clusters": n_clusters,
        "compacted_count": len(compacted),
        "merges": merges,
        "phi_before": phi_before,
        "phi_after": phi_after,
        "pairwise": pairwise,
        "frac_outside": frac_outside,
    }



# ═════════════════════════════════════════════════════════════════════════════
# MAIN
# ═════════════════════════════════════════════════════════════════════════════

if __name__ == "__main__":
    print("=" * 72)
    print("  THE MACHINE — DYNAMICAL SYSTEMS VERIFICATION v2")
    print("  Testing actual failure modes, not algebraic identities.")
    print("=" * 72)
    print()

    rng = random.Random(42)
    test_algebraic_identities(rng)

    # NOTE: trial/hop counts reduced from the original vectorised version
    # (trials 50 → 30, obs 1000 → 500, steps 500 → 300, LSH N 10000 → 5000,
    #  bundling 100 → 50) to keep the pure-Python runtime reasonable.  The
    # physics and all measured quantities are unchanged.
    results_imperfect = measure_epsilon_imperfect(
        max_hops=10,
        bridge_similarities=[0.99, 0.95, 0.90, 0.80, 0.70, 0.60],
        trials=30,
    )
    means = analyze_imperfect_composition(results_imperfect)

    # ── Section 1 verification ──────────────────────────────────────────────
    sigmas = sorted(means.keys(), reverse=True)
    for sigma in sigmas:
        m = means[sigma]
        assert len(m) == 10, f"ε(·,{sigma}) must have 10 hop columns"
        bump()
        for v in m:
            assert 0.0 <= v <= 1.0, f"ε(·,{sigma}) mean must be in [0,1]"
            bump()
        # Closed form at n=1: error equals the bridge flip rate (1-σ)/2.
        assert abs(m[0] - (1.0 - sigma) / 2.0) < 0.02, \
            f"ε(1,{sigma}) must equal (1-σ)/2"
        bump()
        # Error grows with depth.
        assert m[-1] > m[0], f"ε must grow with n for σ={sigma}"
        bump()
        # Boundary crossing logic.
        fail = [h for h in range(len(m)) if m[h] > 0.40]
        if fail:
            assert m[fail[0]] > 0.40 and (fail[0] == 0 or m[fail[0] - 1] <= 0.40), \
                "error-crossing detection must be consistent"
            bump()
        # Deeper hops are decorrelated by the rotation artifact, ~0.5.
        assert 0.2 <= m[-1] <= 0.8, f"deep-hop ε for σ={sigma} should be ~0.5"
        bump()
    # At n=1 higher σ gives strictly lower error.
    for i in range(len(sigmas) - 1):
        hi, lo = sigmas[i], sigmas[i + 1]
        assert means[hi][0] < means[lo][0], f"ε(1,{hi}) < ε(1,{lo})"
        bump()
    # And the whole chain weakly tracks σ (deep columns are saturated, so only
    # the n=1 column is a strict ordering; assert the aggregate trend exists).
    agg_hi = mean(means[sigmas[0]])
    agg_lo = mean(means[sigmas[-1]])
    assert agg_hi <= agg_lo + 0.05, "aggregate ε should not increase with σ"
    bump()

    acc = test_accumulator_asymmetry(obs_count=500)

    # ── Section 2 verification ──────────────────────────────────────────────
    for cfg in acc["configs"]:
        for pc in cfg["popcounts"]:
            assert 0.0 <= pc <= 1.0, "centroid popcount must be in [0,1]"
            bump()
        assert cfg["down"] >= 0 and cfg["up"] >= 0, \
            "flip counts must be non-negative"
        bump()
        # With 50%-density observations the popcount stays near 0.5; it does
        # NOT drift to 1.0.  This is the true relationship.
        assert abs(cfg["final_pc"] - 0.5) < 0.15, \
            "50%-density popcount must stay near 0.5 (no drift to 1.0)"
        bump()
    assert acc["total_down"] > 0, \
        "random absorption must produce at least one 1→0 flip (claim is false)"
    bump()

    sp = test_speciation_timing(steps=300)

    # ── Section 3 verification ──────────────────────────────────────────────
    for row in sp:
        assert 0.0 <= row["saturation"] <= 1.0, "saturation must be in [0,1]"
        bump()
        assert 0.0 <= row["mean_nhd"] <= 1.0, "mean NHD must be in [0,1]"
        bump()
        # For drift ≤ 0.40 the gate never crosses the 0.70 threshold.
        assert row["obs_to_speciate"] == 0, \
            f"drift {row['drift_nhd']} must not trigger the novelty gate"
        bump()
        # NHD(obs, centroid) stays near 0.5 independent of drift.
        assert abs(row["mean_nhd"] - 0.5) < 0.1, \
            f"NHD(obs,centroid) at drift {row['drift_nhd']} should be ~0.5"
        bump()

    lsh = test_lsh_distribution(N=5000)

    # ── Section 4 verification ──────────────────────────────────────────────
    assert sum(lsh["counts"]) == lsh["N"], "LSH counts must sum to N"
    bump()
    assert all(c >= 0 for c in lsh["counts"]), "LSH counts must be non-negative"
    bump()
    assert lsh["max_dev"] == max(abs(c - lsh["expected"]) for c in lsh["counts"]), \
        "reported max deviation must match counts"
    bump()
    assert min(lsh["counts"]) <= lsh["expected"] <= max(lsh["counts"]), \
        "expected count must lie within the observed range"
    bump()
    assert lsh["chi_sq"] < lsh["crit"], \
        f"LSH must be uniform: χ²={lsh['chi_sq']:.2f} < {lsh['crit']}"
    bump()

    bias = test_bundling_bias(trials=50)

    # ── Section 5 verification ──────────────────────────────────────────────
    for row in bias:
        assert abs(row["mean_bias"]) < 0.01, \
            f"bundling bias at n={row['n']} must be < 0.01"
        bump()
        assert math.isfinite(row["std_bias"]) and row["std_bias"] < 0.05, \
            f"bundling std at n={row['n']} must be finite and small"
        bump()
        for bv in row["biases"]:
            assert -0.5 <= bv <= 0.5, "per-trial bias must be in [-0.5,0.5]"
            bump()

    comp = test_compaction_potential()

    # ── Section 6 verification ──────────────────────────────────────────────
    assert 0 <= comp["compacted_count"] <= comp["n_clusters"], \
        "compacted count must be between 0 and initial"
    bump()
    assert math.isfinite(comp["phi_before"]) and math.isfinite(comp["phi_after"]), \
        "Φ values must be finite"
    bump()
    for p in comp["pairwise"]:
        assert 0.0 <= p <= 1.0, "pairwise NHD must be in [0,1]"
        bump()
    # The seeded near-duplicate groups must be close enough to merge.
    assert comp["merges"] > 0, \
        "seeded near-duplicate clusters must trigger NHD<0.30 merges"
    bump()
    assert comp["compacted_count"] < comp["n_clusters"], \
        "merges must reduce the cluster count"
    bump()
    # Merging truly exercises the Φ-decrease mechanism (X.1).
    assert comp["phi_after"] < comp["phi_before"], \
        "compaction must reduce Φ when merges occur"
    bump()
    assert 0.0 <= comp["frac_outside"] <= 1.0, "frac_outside must be in [0,1]"
    bump()
    assert (1.0 - comp["frac_outside"]) > 0.5, \
        "most surviving centroids must lie in the 0.30-0.70 equilibrium band"
    bump()

    print("\n" + "=" * 72)
    print("  STATUS — TRULY PROVEN VS EMPIRICALLY OBSERVED")
    print("=" * 72)
    print("""
  PROVEN (algebraic identity, no assumptions) — now executed above:
    ✓ XOR binding, unbinding, composition are exact in GF(2)
    ✓ Constitutional bundling: order-independent, deterministic
    ✓ Self-reinforcement: centroid is a fixed point
    ✓ Variable rotation: distinct ρ → non-commutative binding

  EMPIRICALLY OBSERVED (measured, not proven):
    ∼ Imperfect bridges (σ < 1.0) cause composition error at n=1: ε(1,σ)=(1-σ)/2
    ∼ ε grows with chain depth n up to the decorrelation floor (~0.5)
    ∼ LSH distribution is uniform (passes χ² test)
    ∼ Bundling is unbiased (majority rule on random inputs)
    ∼ Accumulator popcount stays ~0.5 at 50% density
    ∼ Compaction reduces Φ when near-duplicate clusters merge (seeded groups)

  NOTE — claims in the original narrative that the measurement does NOT support:
    ! "Imperfect bridges cause ε(n,σ)": even a PERFECT bridge returns a
      rotated vector at n≥2, giving ε≈0.5 independent of σ.  The rotation
      artifact dominates deep chains; only n=1 reflects bridge quality.
    """)
    print(f"    ! \"Bits NEVER flip from 1→0\": measured {acc['total_down']} "
          f"downward flips.")
    print(f"    ! \"Popcount drifts toward 1.0\": measured final popcount "
          f"~{acc['configs'][-1]['final_pc']:.3f} at 50% density.")
    print(f"    ~ \"Φ decreases monotonically\": holds for seeded near-duplicate "
          f"groups (ΔΦ={comp['phi_after'] - comp['phi_before']:+.4f}); a purely "
          f"random centroid set never merges, so Φ is then unchanged.")
    print("""
  CRITICAL UNVERIFIED CLAIMS (need formal proof):
    ✗ ε(n, σ) bounds for n > MAX_CHAIN_DEPTH (need actual bound)
    ✗ Novelty gate triggers before centroid saturation guarantees
    ✗ Cluster count bounded by M·(1+S) under all conditions
    ✗ No feedback-induced oscillation (perception→reasoning→action)
    ✗ Phase transitions absent under adversarial input patterns
""")

    print("=" * 72)
    print(f"  {_CHECKS} checks passed")
    print("=" * 72)
