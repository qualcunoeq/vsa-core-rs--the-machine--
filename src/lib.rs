use rand::Rng;
use std::collections::HashMap;

pub mod autonomy;
pub mod forager;
pub mod ledger;
pub mod socket;

pub const HD_DIMENSION: usize = 10048;
pub const U64_BLOCKS: usize = 157;

impl Hypervector {
    pub fn role_market() -> Self {
        Self::encode_text_ngram("ROLE_MARKET_STATE", 3)
    }

    pub fn role_news() -> Self {
        Self::encode_text_ngram("ROLE_NEWS_STATE", 3)
    }

    pub fn role_infra() -> Self {
        Self::encode_text_ngram("ROLE_INFRA_STATE", 3)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hypervector {
    pub bits: [u64; U64_BLOCKS],
}

impl Hypervector {
    pub fn new_random() -> Self {
        let mut rng = rand::thread_rng();
        let mut bits = [0u64; U64_BLOCKS];
        for b in bits.iter_mut() {
            *b = rng.gen();
        }
        Hypervector { bits }
    }

    pub fn new_zero() -> Self {
        Hypervector { bits: [0u64; U64_BLOCKS] }
    }

    pub fn count_ones(&self) -> usize {
        self.bits.iter().map(|b| b.count_ones() as usize).sum()
    }

    pub fn bitwise_xor(&self, other: &Self) -> Self {
        let mut result = [0u64; U64_BLOCKS];
        for i in 0..U64_BLOCKS {
            result[i] = self.bits[i] ^ other.bits[i];
        }
        Hypervector { bits: result }
    }

    pub fn normalized_hamming_distance(&self, other: &Self) -> f64 {
        let mut diff_count = 0;
        for i in 0..U64_BLOCKS {
            let xor_val = self.bits[i] ^ other.bits[i];
            diff_count += xor_val.count_ones(); // Native CPU popcount
        }
        (diff_count as f64) / (HD_DIMENSION as f64)
    }

    pub fn rotate_left(&self, shift: usize) -> Self {
        let shift = shift % HD_DIMENSION;
        if shift == 0 {
            return *self;
        }
        let word_shift = shift / 64;
        let bit_shift = shift % 64;
        
        let mut result = [0u64; U64_BLOCKS];
        for i in 0..U64_BLOCKS {
            let src_idx1 = (i + word_shift) % U64_BLOCKS;
            let src_idx2 = (i + word_shift + 1) % U64_BLOCKS;
            
            if bit_shift == 0 {
                result[i] = self.bits[src_idx1];
            } else {
                let part1 = self.bits[src_idx1] >> bit_shift;
                let part2 = self.bits[src_idx2] << (64 - bit_shift);
                result[i] = part1 | part2;
            }
        }
        Hypervector { bits: result }
    }

    /// Proprietary Chaotic Shift-XOR Character Encoder
    pub fn encode_char(c: char, index_seed: usize) -> Self {
        let mut hv = [0u64; U64_BLOCKS];
        let char_val = c as u64;
        
        for i in 0..U64_BLOCKS {
            // A proprietary LCG/Xorshift cascade to generate pseudo-random bits
            let mut x = char_val.wrapping_add(i as u64).wrapping_mul(0x9E3779B97F4A7C15);
            x ^= x >> 30;
            x = x.wrapping_mul(0xBF58476D1CE4E5B9);
            x ^= x >> 27;
            hv[i] = x ^ (index_seed as u64);
        }
        Hypervector { bits: hv }
    }

    /// Sequence-Preserving N-Gram Permutation with Page-Level Majority Bundling
    pub fn encode_text_ngram(text: &str, k: usize) -> Self {
        if text.is_empty() {
            return Self::new_zero();
        }
        
        let chars: Vec<char> = text.chars().collect();
        if chars.len() < k {
            let mut ngram_vec = Self::new_zero();
            for (i, &c) in chars.iter().enumerate() {
                let char_vec = Self::encode_char(c, i);
                let rotated = char_vec.rotate_left(i);
                if i == 0 {
                    ngram_vec = rotated;
                } else {
                    ngram_vec = ngram_vec.bitwise_xor(&rotated);
                }
            }
            return ngram_vec;
        }
        
        let mut ngrams = Vec::new();
        for i in 0..=(chars.len() - k) {
            let mut ngram_vec = Self::new_zero();
            for j in 0..k {
                let char_vec = Self::encode_char(chars[i + j], j);
                let rotated = char_vec.rotate_left(j);
                if j == 0 {
                    ngram_vec = rotated;
                } else {
                    ngram_vec = ngram_vec.bitwise_xor(&rotated);
                }
            }
            ngrams.push(ngram_vec);
        }
        
        let refs: Vec<&Hypervector> = ngrams.iter().collect();
        Self::bundle(&refs)
    }

    /// Bit-Parallel Majority Bundling with noise injection for tie-breaking
    pub fn bundle(vectors: &[&Self]) -> Self {
        if vectors.is_empty() { return Self::new_zero(); }
        if vectors.len() == 1 { return *vectors[0]; }

        let mut result_bits = [0u64; U64_BLOCKS];
        let num_vectors = vectors.len();
        let halfway = num_vectors / 2;
        let is_even = num_vectors % 2 == 0;
        
        let noise_vector = if is_even {
            Self::new_random()
        } else {
            Self::new_zero()
        };

        for block_idx in 0..U64_BLOCKS {
            let mut block_consensus = 0u64;
            for bit_idx in 0..64 {
                let mut bit_count = 0;
                for vec in vectors {
                    if ((vec.bits[block_idx] >> bit_idx) & 1) == 1 {
                        bit_count += 1;
                    }
                }
                
                if is_even && bit_count == halfway {
                    let noise_bit = ((noise_vector.bits[block_idx] >> bit_idx) & 1) == 1;
                    if noise_bit {
                        block_consensus |= 1 << bit_idx;
                    }
                } else if bit_count > halfway {
                    block_consensus |= 1 << bit_idx;
                }
            }
            result_bits[block_idx] = block_consensus;
        }
        Hypervector { bits: result_bits }
    }

    /// Continuous Value Interpolation Mapping
    pub fn encode_continuous(config: &VarConfig, val: f64) -> Self {
        let clamped = val.clamp(config.min_val, config.max_val);
        let fraction = (clamped - config.min_val) / (config.max_val - config.min_val);
        let num_bits_max = (fraction * (HD_DIMENSION as f64)).round() as usize;
        let num_bits_min = HD_DIMENSION - num_bits_max;

        let mut result = [0u64; U64_BLOCKS];
        for i in 0..U64_BLOCKS {
            let start_bit = i * 64;
            let end_bit = start_bit + 64;

            if end_bit <= num_bits_min {
                result[i] = config.base_min.bits[i];
            } else if start_bit >= num_bits_min {
                result[i] = config.base_max.bits[i];
            } else {
                let split = num_bits_min - start_bit;
                let mask_min = (1u64 << split) - 1;
                let part_min = config.base_min.bits[i] & mask_min;
                let part_max = (config.base_max.bits[i] >> split) << split;
                result[i] = part_min | part_max;
            }
        }
        Hypervector { bits: result }
    }

    /// Convert Hypervector to a raw 1256-byte buffer (using 157 * 8 bytes = 1256 bytes)
    pub fn to_bytes(&self) -> [u8; 1256] {
        let mut bytes = [0u8; 1256];
        for i in 0..157 {
            let block_bytes = self.bits[i].to_le_bytes();
            bytes[i * 8 .. (i + 1) * 8].copy_from_slice(&block_bytes);
        }
        bytes
    }

    /// Parse Hypervector from a raw 1256-byte buffer
    pub fn from_bytes(bytes: &[u8; 1256]) -> Self {
        let mut bits = [0u64; U64_BLOCKS];
        for i in 0..157 {
            let mut block_bytes = [0u8; 8];
            block_bytes.copy_from_slice(&bytes[i * 8 .. (i + 1) * 8]);
            bits[i] = u64::from_le_bytes(block_bytes);
        }
        Hypervector { bits }
    }
}

#[derive(Clone, Debug)]
pub struct VarConfig {
    pub id: Hypervector,
    pub min_val: f64,
    pub max_val: f64,
    pub base_min: Hypervector,
    pub base_max: Hypervector,
}

#[derive(Clone, Debug)]
pub struct DejavuEntry {
    pub vector: Hypervector,
    pub label: String,
    pub metadata: HashMap<String, String>,
}

pub struct VSABrain {
    pub variables: HashMap<String, VarConfig>,
    pub concepts: HashMap<String, Hypervector>,
    pub dejavu_db: Vec<DejavuEntry>,
    pub threshold: f64,
}

impl VSABrain {
    pub fn new(threshold: f64) -> Self {
        VSABrain {
            variables: HashMap::new(),
            concepts: HashMap::new(),
            dejavu_db: Vec::new(),
            threshold,
        }
    }

    pub fn generate_vector(&self) -> Hypervector {
        Hypervector::new_random()
    }

    pub fn bind(&self, v1: &Hypervector, v2: &Hypervector) -> Hypervector {
        v1.bitwise_xor(v2)
    }

    pub fn unbind(&self, v1: &Hypervector, v2: &Hypervector) -> Hypervector {
        v1.bitwise_xor(v2)
    }

    pub fn similarity(&self, v1: &Hypervector, v2: &Hypervector) -> f64 {
        1.0 - v1.normalized_hamming_distance(v2)
    }

    pub fn register_variable(&mut self, name: &str, min_val: f64, max_val: f64) {
        let id = Hypervector::new_random();
        let base_min = Hypervector::new_random();
        let base_max = Hypervector::new_random();
        self.variables.insert(name.to_string(), VarConfig {
            id,
            min_val,
            max_val,
            base_min,
            base_max,
        });
    }

    pub fn register_concept(&mut self, name: &str) -> Hypervector {
        let vec = Hypervector::new_random();
        self.concepts.insert(name.to_string(), vec);
        vec
    }

    pub fn encode_continuous(&self, name: &str, val: f64) -> Option<Hypervector> {
        let config = self.variables.get(name)?;
        Some(Hypervector::encode_continuous(config, val))
    }

    pub fn encode_and_bind_variable(&self, name: &str, val: f64) -> Option<Hypervector> {
        let config = self.variables.get(name)?;
        let val_vector = Hypervector::encode_continuous(config, val);
        Some(config.id.bitwise_xor(&val_vector))
    }

    pub fn bundle(&self, vectors: &[Hypervector]) -> Hypervector {
        let refs: Vec<&Hypervector> = vectors.iter().collect();
        Hypervector::bundle(&refs)
    }

    pub fn compile_state_vector(&self, telemetry: &HashMap<String, f64>) -> Hypervector {
        let mut bound_vectors = Vec::new();
        for (name, val) in telemetry {
            if let Some(bound) = self.encode_and_bind_variable(name, *val) {
                bound_vectors.push(bound);
            }
        }
        let refs: Vec<&Hypervector> = bound_vectors.iter().collect();
        Hypervector::bundle(&refs)
    }

    pub fn add_to_dejavu_db(&mut self, vector: Hypervector, label: &str, metadata: HashMap<String, String>) {
        self.dejavu_db.push(DejavuEntry {
            vector,
            label: label.to_string(),
            metadata,
        });
    }

    pub fn query_dejavu(&self, vector: &Hypervector) -> (Option<String>, f64, HashMap<String, String>) {
        if self.dejavu_db.is_empty() {
            return (None, 0.0, HashMap::new());
        }
        let mut best_label = None;
        let mut best_sim = -1.0;
        let mut best_meta = HashMap::new();

        for entry in &self.dejavu_db {
            let sim = 1.0 - vector.normalized_hamming_distance(&entry.vector);
            if sim > best_sim {
                best_sim = sim;
                best_label = Some(entry.label.clone());
                best_meta = entry.metadata.clone();
            }
        }
        (best_label, best_sim, best_meta)
    }

    pub fn evaluate_deja_vu(&self, vector: &Hypervector) -> (Option<String>, f64) {
        let mut best_label = None;
        let mut min_dist = 1.0;

        for entry in &self.dejavu_db {
            let dist = vector.normalized_hamming_distance(&entry.vector);
            if dist < min_dist {
                min_dist = dist;
                best_label = Some(entry.label.clone());
            }
        }
        
        if min_dist <= self.threshold {
            (best_label, min_dist)
        } else {
            (None, min_dist)
        }
    }

    pub fn decode_variable(&self, state_vector: &Hypervector, var_name: &str, resolution: usize) -> Option<f64> {
        let config = self.variables.get(var_name)?;
        let unbound = state_vector.bitwise_xor(&config.id);
        
        let mut best_val = config.min_val;
        let mut max_sim = -1.0;
        
        for step in 0..=resolution {
            let fraction = (step as f64) / (resolution as f64);
            let val = config.min_val + fraction * (config.max_val - config.min_val);
            let encoded = Hypervector::encode_continuous(config, val);
            
            let sim = 1.0 - unbound.normalized_hamming_distance(&encoded);
            if sim > max_sim {
                max_sim = sim;
                best_val = val;
            }
        }
        Some(best_val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hamming_distance_ident() {
        let v1 = Hypervector::new_random();
        assert_eq!(v1.normalized_hamming_distance(&v1), 0.0);
        let v2 = Hypervector::new_random();
        let dist = v1.normalized_hamming_distance(&v2);
        assert!(dist > 0.40 && dist < 0.60, "Hamming distance of random vectors should be around 0.5, got {}", dist);
    }

    #[test]
    fn test_char_encoder() {
        let v1 = Hypervector::encode_char('a', 42);
        let v2 = Hypervector::encode_char('a', 42);
        let v3 = Hypervector::encode_char('b', 42);
        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    #[test]
    fn test_ledger_serialization() {
        let v1 = Hypervector::new_random();
        let bytes = v1.to_bytes_1250();
        let v2 = Hypervector::from_bytes_1250(&bytes);
        for i in 0..156 {
            assert_eq!(v1.bits[i], v2.bits[i]);
        }
        let mask = (1u64 << 16) - 1;
        assert_eq!(v1.bits[156] & mask, v2.bits[156] & mask);
    }

    #[test]
    fn test_continuous_encoding() {
        let config = VarConfig {
            id: Hypervector::new_random(),
            min_val: -3.0,
            max_val: 3.0,
            base_min: Hypervector::new_random(),
            base_max: Hypervector::new_random(),
        };
        let v_min = Hypervector::encode_continuous(&config, -3.0);
        let v_max = Hypervector::encode_continuous(&config, 3.0);
        let v_mid = Hypervector::encode_continuous(&config, 0.0);

        let d_min_max = v_min.normalized_hamming_distance(&v_max);
        let d_min_mid = v_min.normalized_hamming_distance(&v_mid);
        let d_mid_max = v_mid.normalized_hamming_distance(&v_max);

        // Distance should scale linearly
        assert!(d_min_mid < d_min_max);
        assert!(d_mid_max < d_min_max);
    }
}
