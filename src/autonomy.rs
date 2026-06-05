use crate::Hypervector;

#[derive(Clone, Debug)]
pub struct AutonomyDrive {
    pub dissonance_threshold: f64,
}

impl AutonomyDrive {
    pub fn new(dissonance_threshold: f64) -> Self {
        AutonomyDrive {
            dissonance_threshold,
        }
    }

    /// Evaluates the discrepancy vector between expectation (historical memory)
    /// and current reality (active world state) via XOR.
    pub fn calculate_dissonance(current: &Hypervector, historical: &Hypervector) -> Hypervector {
        current.bitwise_xor(historical)
    }

    /// Evaluates whether the discrepancy is a meaningful structural anomaly
    /// that justifies pivoting active search intent.
    pub fn evaluates_necessity_to_pivot(&self, dissonance: &Hypervector) -> bool {
        let set_bits = dissonance.count_ones();
        let normalized_dist = set_bits as f64 / 10048.0;

        normalized_dist > self.dissonance_threshold && normalized_dist < 0.55
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_dissonance() {
        let v1 = Hypervector::new_random();
        let v2 = Hypervector::new_random();
        let dissonance = AutonomyDrive::calculate_dissonance(&v1, &v2);

        // XOR is self-reversible: dissonance XOR v1 should yield v2
        let reversed = dissonance.bitwise_xor(&v1);
        assert_eq!(reversed, v2);
    }

    #[test]
    fn test_necessity_to_pivot() {
        let drive = AutonomyDrive::new(0.43);

        // 1. Identical vectors -> dissonance is zero -> no pivot
        let v1 = Hypervector::new_random();
        let diss_zero = AutonomyDrive::calculate_dissonance(&v1, &v1);
        assert!(!drive.evaluates_necessity_to_pivot(&diss_zero));

        // 2. Completely random vectors -> distance ~0.50 -> should pivot if above 0.43 and below 0.55
        let v2 = Hypervector::new_random();
        let diss_random = AutonomyDrive::calculate_dissonance(&v1, &v2);
        let dist = diss_random.normalized_hamming_distance(&Hypervector::new_zero());
        if dist > 0.43 && dist < 0.55 {
            assert!(drive.evaluates_necessity_to_pivot(&diss_random));
        }
    }
}
