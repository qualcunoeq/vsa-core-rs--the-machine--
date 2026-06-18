#!/usr/bin/env python3
"""
Theorem XXII.1-R: Corrected Adversarial L_F Bound
=================================================
The original proof claimed L_F ≤ 0.5. This is WRONG. The correct bound is
L_F ≤ 1.0, with L_F = 1.0 achievable by a specific adversarial construction.

We prove:
  (1) L_F = sup_{v≠v'} W₁(F(M,v), F(M,v')) / δ(v,v') ≤ 1.0 always
  (2) L_F = 1.0 is achievable (the original bound 0.5 was too strict)
  (3) Even L_F = 1.0 does NOT break joint contraction (margin still positive)

References:
  - MATH.md Section XXII (erroneous proof claiming L_F ≤ 0.5)
  - src/reason.rs lines 2878-3027 (test_adversarial_lf)
  - src/lib.rs lines 685-704 (absorb_entry)
  - src/lib.rs lines 711-726 (recompute_centroid: threshold = floor(W/2))
"""

import math
import random
import statistics
from typing import List, Tuple

D = 10240
γ = 0.975
DECAY_INTERVAL = 50
W_MAX = 500
SEED = 42

# α = 3, κ_P = 0.68, β = 1, κ_F = 0.95 (from Corollary XXII.1)
ALPHA = 3.0
KAPPA_P = 0.68
BETA = 1.0
KAPPA_F = 0.95


class SingleBitAccumulator:
    """Exact reproduction of the Rust accumulator logic for one bit."""
    
    def __init__(self, initial_acc: int, initial_weight: int):
        self.acc = initial_acc
        self.W = initial_weight
        
    def centroid_bit(self) -> int:
        """Rust: centroid[i] = 1 iff acc[i] > floor(W/2)"""
        return 1 if self.acc > (self.W // 2) else 0

    def margin(self) -> float:
        """m = a - floor(W/2)"""
        return self.acc - (self.W // 2)
    
    def absorb(self, tau_bit: int):
        """Rust absorb_entry (lib.rs:685-704)"""
        self.acc += tau_bit
        self.W += 1
        if self.W > W_MAX:
            scale = W_MAX / self.W
            self.acc = round(self.acc * scale)
            self.W = W_MAX


def run_adversarial_Lf_test():
    """
    Construct the exact worst case:
    1. Send 50 all-1s observations → A_i = 50 for all i, W = 50
    2. Send 50 all-0s observations → A_i stays 50 for all i, W = 100
       Now all bits have A_i = floor(100/2) = 50, centroid = all-0s
    3. Compare: absorb all-1s vs absorb all-0s
       Measure L_F = δ(new_c_v, new_c_v') / δ(v, v')
    """
    print("=" * 72)
    print("  THEOREM XXII.1-R: CORRECTED ADVERSARIAL L_F BOUND")
    print("=" * 72)

    print("""
  The original proof in MATH.md Section XXII claimed L_F ≤ 0.5.
  This is incorrect. The correct bound is L_F ≤ 1.0, with equality
  achievable by:
    
    Phase 1 (setup): Send 50 all-1s observations
      → A_i = 50 for all i, W = 50, centroid = all-1s
    
    Phase 2 (boundary preparation): Send 50 all-0s observations
      → A_i stays 50, W = 100, centroid = all-0s
      → All D bits have |A_i - floor(W/2)| = 0 (maximally fragile)
    
    Phase 3 (L_F measurement): Compare two counterfactual absorptions
      v  = all-1s → A_i + 1 = 51 > floor(101/2)=50 → centroid = all-1s
      v' = all-0s → A_i + 0 = 50 > floor(101/2)=50 → centroid = all-0s
      δ(new_v, new_v') = 1.0,  δ(v, v') = 1.0,  L_F = 1.0
  """)

    # ═══════════════════════════════════════════════════════════════
    # PART 1: Exact adversarial construction (single bit representative)
    # ═══════════════════════════════════════════════════════════════

    print("  Part 1: Exact adversarial construction")
    print("  ──────────────────────────────────────")
    
    # Phase 1: 50 all-1s
    acc = SingleBitAccumulator(0, 0)
    for _ in range(50):
        acc.absorb(1)
    a1, w1 = acc.acc, acc.W
    c1 = acc.centroid_bit()
    print(f"  Phase 1 (50× all-1s): a={a1}, W={w1}, centroid={c1}")
    
    # Phase 2: 50 all-0s
    for _ in range(50):
        acc.absorb(0)
    a2, w2 = acc.acc, acc.W
    c2 = acc.centroid_bit()
    m2 = acc.margin()
    print(f"  Phase 2 (50× all-0s): a={a2}, W={w2}, centroid={c2}, margin={m2}")
    
    # Phase 3a: absorb all-1s
    acc1 = SingleBitAccumulator(a2, w2)
    acc1.absorb(1)
    print(f"  Phase 3a (absorb all-1s): a={acc1.acc}, W={acc1.W}, centroid={acc1.centroid_bit()}")
    
    # Phase 3b: absorb all-0s
    acc0 = SingleBitAccumulator(a2, w2)
    acc0.absorb(0)
    print(f"  Phase 3b (absorb all-0s): a={acc0.acc}, W={acc0.W}, centroid={acc0.centroid_bit()}")
    
    # L_F = δ(new_v, new_v') / δ(v, v')
    # Since we're looking at one bit, δ = 0 or 1
    delta_centroid = abs(acc1.centroid_bit() - acc0.centroid_bit())
    delta_input = 1.0  # all-1s vs all-0s differ on this bit
    L_F_single = delta_centroid / delta_input if delta_input > 0 else 0
    print(f"\n  Single-bit L_F = {delta_centroid}/{delta_input} = {L_F_single}")
    print(f"  (Extends to all D bits: L_F = 1.0)")
    
    # ═══════════════════════════════════════════════════════════════
    # PART 2: Full D-dimensional simulation
    # ═══════════════════════════════════════════════════════════════

    print("\n  Part 2: Full D-dimensional adversarial simulation")
    print("  ────────────────────────────────────────────────")
    
    rng = random.Random(SEED)
    
    # We track each of D bits individually
    a = [0] * D
    W = 0
    
    # Phase 1: 50 all-1s
    for _ in range(50):
        for i in range(D):
            a[i] += 1
        W += 1
    
    # Compute centroid
    threshold = W // 2
    centroid_after_p1 = [1 if a[i] > threshold else 0 for i in range(D)]
    pop_p1 = sum(centroid_after_p1) / D
    print(f"  After Phase 1: W={W}, centroid popcount={pop_p1:.4f}")
    
    # Phase 2: 50 all-0s
    for _ in range(50):
        for i in range(D):
            a[i] += 0  # no change
        W += 1
    
    threshold = W // 2
    centroid_after_p2 = [1 if a[i] > threshold else 0 for i in range(D)]
    pop_p2 = sum(centroid_after_p2) / D
    margins = [a[i] - (W // 2) for i in range(D)]
    min_margin = min(margins)
    max_margin = max(margins)
    boundary_bits = sum(1 for m in margins if abs(m) <= 1)
    print(f"  After Phase 2: W={W}, centroid popcount={pop_p2:.4f}")
    print(f"  Margin range: [{min_margin}, {max_margin}], boundary bits: {boundary_bits}/{D}")
    
    # Phase 3a: absorb all-1s into a COPY
    a1 = a.copy()
    W1 = W
    for i in range(D):
        a1[i] += 1
    W1 += 1
    threshold1 = W1 // 2
    centroid1 = [1 if a1[i] > threshold1 else 0 for i in range(D)]
    
    # Phase 3b: absorb all-0s into another COPY
    a0 = a.copy()
    W0 = W
    for i in range(D):
        a0[i] += 0
    W0 += 1
    threshold0 = W0 // 2
    centroid0 = [1 if a0[i] > threshold0 else 0 for i in range(D)]
    
    # L_F computation
    differing_bits = sum(1 for i in range(D) if centroid1[i] != centroid0[i])
    delta_centroid = differing_bits / D
    delta_input = 1.0  # all-1s vs all-0s: all D bits differ
    L_F_achieved = delta_centroid / delta_input
    
    print(f"\n  Phase 3: L_F measurement")
    print(f"  δ(new_v, new_v') = {differing_bits}/{D} = {delta_centroid:.6f}")
    print(f"  δ(v, v')         = {delta_input}")
    print(f"  L_F achieved     = {L_F_achieved:.6f}")
    print(f"  Original claim   = 0.5 (INCORRECT — proof error)")
    print(f"  Correct bound    = 1.0 (tight — achieved here)")
    
    # ═══════════════════════════════════════════════════════════════
    # PART 3: Joint contraction check
    # ═══════════════════════════════════════════════════════════════

    print("\n  Part 3: Joint contraction check")
    print("  ───────────────────────────────")
    
    left = ALPHA * (1 - KAPPA_P)
    right = BETA * KAPPA_F * L_F_achieved
    margin = left - right
    
    print(f"  Left:  α·(1-κ_P) = {ALPHA}·{1-KAPPA_P:.2f} = {left:.4f}")
    print(f"  Right: β·κ_F·L_F = {BETA}·{KAPPA_F}·{L_F_achieved:.4f} = {right:.4f}")
    print(f"  Margin: {margin:.4f}")
    print(f"  Joint contraction: {'✓ HOLDS' if margin > 0 else '✗ FAILS'}")
    
    # Find critical L_F that WOULD break joint contraction
    L_F_critical = left / (BETA * KAPPA_F)
    print(f"\n  Critical L_F (would break joint contraction): {L_F_critical:.4f}")
    print(f"  Is L_F = 1.0 achievable? YES (proven above)")
    print(f"  Is L_F = {L_F_critical:.4f} achievable? ", end="")
    
    # Can L_F exceed the critical value?
    # L_F = δ(new_v, new_v') / δ(v, v') ≤ 1.0 always (bit-wise subset proof)
    if L_F_critical > 1.0:
        print(f"NO — L_F is bounded by 1.0, and 1.0 < {L_F_critical:.4f}")
        print(f"  → Joint contraction holds for ALL possible inputs")
    else:
        print(f"YES — L_F can reach {L_F_critical:.4f} if boundary condition met")
    
    # ═══════════════════════════════════════════════════════════════
    # PART 4: Proof that L_F ≤ 1.0 always
    # ═══════════════════════════════════════════════════════════════

    print("\n  Part 4: Proof of L_F ≤ 1.0 (the correct bound)")
    print("  ─────────────────────────────────────────────")
    print("""
  For each bit i:
    Let c_v[i] = 1{A_i + v_i > floor((W+1)/2)}
    Let c_v'[i] = 1{A_i + v'_i > floor((W+1)/2)}
  
  Cases:
    (a) v_i = v'_i = 0:  c_v[i] = c_v'[i] = 1{A_i > floor((W+1)/2)}
                         → Δ_i = 0
    
    (b) v_i = v'_i = 1:  c_v[i] = c_v'[i] = 1{A_i + 1 > floor((W+1)/2)}
                         → Δ_i = 0
    
    (c) v_i = 1, v'_i = 0:
        Δ_i = |1{A_i + 1 > T_new} − 1{A_i > T_new}|
        Δ_i = 1 iff A_i ∈ (T_new − 1, T_new]  (i.e., A_i = T_new with integer)
        → Δ_i ≤ 1 always
        → Δ_i ≤ 1{v_i ≠ v'_i} always (since (c) requires v_i ≠ v'_i)
    
  Therefore: ΣΔ_i ≤ Σ1{v_i ≠ v'_i} = D·δ(v, v')
  So δ(new_v, new_v') ≤ δ(v, v') for all v, v', A, W.
  Hence L_F = sup δ(new_v, new_v') / δ(v, v') ≤ 1.0.
  
  Equality (L_F = 1.0) achieved when:
    1. A_i = floor(W/2) for all i (every bit at the boundary)
    2. v_i ≠ v'_i for all i (maximally different inputs)
    3. Then all D bits flip in the centroid: δ(new_v, new_v') = δ(v, v') = 1
  """)

    # ═══════════════════════════════════════════════════════════════
    # PART 5: Compare with the existing Rust test
    # ═══════════════════════════════════════════════════════════════

    print("  Part 5: Why the Rust test_adversarial_lf only finds L_F ≈ 0.502")
    print("  ─────────────────────────────────────────────────────────────")
    print("""
  The existing test (reason.rs:2886) uses a fixed set of 20 random
  adversarial vectors, picking the one FARTHEST from the current centroid.
  This does NOT find L_F = 1.0 because:
    
    1. Random vectors at 50% density have NHD ≈ 0.5 from any centroid
       → δ(v, v') between two random vectors ≈ 0.5 (not 1.0)
    
    2. The test never constructs the critical boundary condition
       A_i = floor(W/2) for all i simultaneously
    
    3. The centroid never gets "worse case prepared" — the adversary
       needs 100 carefully crafted observations BEFORE the L_F test
  
  A proper adversarial test would:
    1. Set up: 50 all-1s, 50 all-0s → A_i = W/2 for all i
    2. Compare: absorb all-1s vs absorb all-0s
    3. Result: δ(new_v, new_v') = 1.0, δ(v, v') = 1.0, L_F = 1.0
  """)

    # ═══════════════════════════════════════════════════════════════
    # PART 6: Statistical verification — random vs structured adversary
    # ═══════════════════════════════════════════════════════════════

    print("  Part 6: Statistical verification — random vs structured adversary")
    print("  ───────────────────────────────────────────────────────────────")
    
    # Random adversary (like the Rust test)
    print("  Random adversary (Rust test_adversarial_lf method):")
    rng = random.Random(SEED + 1)
    max_lf_random = 0.0
    lf_samples = []
    
    # Start with random centroid
    centroid = [rng.randint(0, 1) for _ in range(D)]
    W = 100
    a = [centroid[i] * 50 + rng.randint(-5, 5) for i in range(D)]
    a = [max(0, min(W, x)) for x in a]
    
    for step in range(200):
        # Pick farthest from current centroid (same strategy as Rust test)
        best_dist = 0
        adv_vec = [0] * D
        for _ in range(20):
            candidate = [rng.randint(0, 1) for _ in range(D)]
            d = sum(1 for i in range(D) if candidate[i] != centroid[i]) / D
            if d > best_dist:
                best_dist = d
                adv_vec = candidate
        
        # Absorb and measure
        a_prev = a.copy()
        W_prev = W
        for i in range(D):
            a_prev[i] += adv_vec[i]
        W_prev += 1
        new_bit = [1 if a_prev[i] > (W_prev // 2) else 0 for i in range(D)]
        
        delta_m = sum(1 for i in range(D) if centroid[i] != new_bit[i]) / D
        delta_v = best_dist
        lf = delta_m / delta_v if delta_v > 0.001 else 0
        if lf > max_lf_random:
            max_lf_random = lf
        lf_samples.append(lf)
        
        centroid = new_bit
        a = a_prev
        W = W_prev
    
    mean_lf = statistics.mean(lf_samples)
    print(f"    Max L_F = {max_lf_random:.4f}, Mean L_F = {mean_lf:.4f}")
    
    # Structured adversary (our construction)
    print("  Structured adversary (boundary construction):")
    print(f"    Max L_F = {L_F_achieved:.4f} (theoretical max: 1.0)")
    print()
    
    print(f"  Ratio: structured/random = {L_F_achieved / max_lf_random:.1f}× worse")
    print(f"  → The Rust test underestimates L_F by {1/max_lf_random:.1f}×")

    # ═══════════════════════════════════════════════════════════════
    # SUMMARY
    # ═══════════════════════════════════════════════════════════════
    
    print("=" * 72)
    print("  THEOREM XXII.1-R: CORRECTED STATEMENT")
    print("=" * 72)
    print(f"""
  Theorem XXII.1-R (Corrected L_F Bound):
    For the integer accumulator with weight W ≥ 1:
    
      L_F ≤ 1.0
    
    with equality achievable when:
      - A_i = ⌊W/2⌋ for all i (all bits at the decision boundary)
      - v_i ≠ v'_i for all i
    
    This is TIGHT: L_F cannot exceed 1.0 because for each bit i:
      Δ_i = 1 only if v_i ≠ v'_i (subset property)
      Therefore δ(new_v, new_v') ≤ δ(v, v') always.
    
  Corollary XXII.1-R (Joint contraction still holds):
    With L_F = 1.0 (worst case):
      α·(1-κ_P) = 3·0.32 = {left:.4f}
      β·κ_F·L_F = 1·0.95·1.0 = {BETA * KAPPA_F * 1.0:.4f}
      Margin = {left - BETA * KAPPA_F * 1.0:.4f} > 0  ✓
    
    The joint contraction condition is satisfied even at L_F = 1.0.
    The safety margin is {margin:.4f} (was claimed as 0.485 in the
    original proof, but the actual margin at the tight bound L_F=1.0
    is {left - BETA * KAPPA_F * 1.0:.4f}).
    
  Implication:
    The original bound L_F ≤ 0.5 was too strict. The correct bound
    is L_F ≤ 1.0. However, this DOES NOT weaken the system — the
    joint contraction condition still holds with margin {left - BETA * KAPPA_F * 1.0:.4f}.
    
    The existing Rust test (test_adversarial_lf) fails to find L_F > 0.502
    because it uses random adversarial vectors. A structured adversary
    (boundary preparation + all-1s vs all-0s) achieves L_F = 1.0.
    
  Action Required:
    The MATH.md proof needs to be corrected. The Rust test can remain
    as-is (it tests the random-case bound, which is ∼0.5), but should
    be supplemented with a structured adversarial test.
""")

    return L_F_achieved, margin


if __name__ == "__main__":
    L_F, margin = run_adversarial_Lf_test()
