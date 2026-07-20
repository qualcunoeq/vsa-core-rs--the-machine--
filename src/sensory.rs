use crate::{Hypervector, VarConfig, FPE_RESOLUTION};
use std::collections::HashMap;

pub trait SensoryModality: Send + Sync {
    fn encode(&self) -> Hypervector;
    fn name(&self) -> &str;
}

pub struct TextSensoryModality {
    pub text: String,
    pub name: String,
}

impl TextSensoryModality {
    pub fn new(name: &str, text: &str) -> Self {
        TextSensoryModality {
            text: text.to_string(),
            name: name.to_string(),
        }
    }
}

impl SensoryModality for TextSensoryModality {
    fn encode(&self) -> Hypervector {
        Hypervector::encode_sentence(&self.text)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// ██ UPGRADE v2.0: FPE-based telemetry modality ██
pub struct SystemTelemetryModality {
    pub name: String,
    pub variables: HashMap<String, VarConfig>,
    pub readings: HashMap<String, f64>,
}

impl SystemTelemetryModality {
    pub fn new(name: &str) -> Self {
        let mut variables = HashMap::new();
        // Register CPU (0 to 100) with FPE levels
        variables.insert(
            "cpu_utilization".to_string(),
            VarConfig {
                id: Hypervector::new_random(),
                min_val: 0.0,
                max_val: 100.0,
                level_vectors: Hypervector::generate_level_vectors(FPE_RESOLUTION),
            },
        );
        // Register RAM Free (0 to 64GB)
        variables.insert(
            "ram_free_gb".to_string(),
            VarConfig {
                id: Hypervector::new_random(),
                min_val: 0.0,
                max_val: 64.0,
                level_vectors: Hypervector::generate_level_vectors(FPE_RESOLUTION),
            },
        );

        SystemTelemetryModality {
            name: name.to_string(),
            variables,
            readings: HashMap::new(),
        }
    }

    pub fn set_reading(&mut self, key: &str, value: f64) {
        self.readings.insert(key.to_string(), value);
    }
}

impl SensoryModality for SystemTelemetryModality {
    fn encode(&self) -> Hypervector {
        let mut bound_vectors = Vec::new();
        for (key, config) in &self.variables {
            let val = self.readings.get(key).cloned().unwrap_or(config.min_val);
            let encoded_val =
                Hypervector::encode_fpe(&config.level_vectors, val, config.min_val, config.max_val);
            bound_vectors.push(config.id.bitwise_xor(&encoded_val));
        }
        let refs: Vec<&Hypervector> = bound_vectors.iter().collect();
        Hypervector::bundle(&refs)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// ██ UPGRADE v2.0: FPE-based network modality ██
pub struct NetworkTrafficModality {
    pub name: String,
    pub active_connections: usize,
    pub bandwidth_mbps: f64,
    pub conn_config: VarConfig,
    pub bw_config: VarConfig,
}

impl NetworkTrafficModality {
    pub fn new(name: &str) -> Self {
        NetworkTrafficModality {
            name: name.to_string(),
            active_connections: 0,
            bandwidth_mbps: 0.0,
            conn_config: VarConfig {
                id: Hypervector::new_random(),
                min_val: 0.0,
                max_val: 1000.0,
                level_vectors: Hypervector::generate_level_vectors(FPE_RESOLUTION),
            },
            bw_config: VarConfig {
                id: Hypervector::new_random(),
                min_val: 0.0,
                max_val: 10000.0,
                level_vectors: Hypervector::generate_level_vectors(FPE_RESOLUTION),
            },
        }
    }
}

impl SensoryModality for NetworkTrafficModality {
    fn encode(&self) -> Hypervector {
        let conn_vec = Hypervector::encode_fpe(
            &self.conn_config.level_vectors,
            self.active_connections as f64,
            self.conn_config.min_val,
            self.conn_config.max_val,
        );
        let bound_conn = self.conn_config.id.bitwise_xor(&conn_vec);

        let bw_vec = Hypervector::encode_fpe(
            &self.bw_config.level_vectors,
            self.bandwidth_mbps,
            self.bw_config.min_val,
            self.bw_config.max_val,
        );
        let bound_bw = self.bw_config.id.bitwise_xor(&bw_vec);

        Hypervector::bundle(&[&bound_conn, &bound_bw])
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ══════════════════════════════════════════════════════════════════════════
// UNIVERSAL SENSORY ENCODERS (Multimodal HDC for General-Purpose Machine)
// ══════════════════════════════════════════════════════════════════════════

/// A random projection matrix for mapping external feature vectors into the
/// hyperdimensional space. This implements the Johnson-Lindenstrauss lemma
/// for dimensionality reduction into the HD space.
///
/// Each input dimension gets a random hypervector. The output is the
/// majority-vote (or bundle) of those hypervectors weighted by the input values.
pub struct RandomProjectionHDC {
    /// Pre-generated random hypervectors for each input dimension
    projection_vectors: Vec<Hypervector>,
    /// Number of input dimensions
    input_dim: usize,
}

impl RandomProjectionHDC {
    /// Create a new random projection with `input_dim` dimensions.
    pub fn new(input_dim: usize) -> Self {
        let mut projection_vectors = Vec::with_capacity(input_dim);
        for _ in 0..input_dim {
            projection_vectors.push(Hypervector::new_random());
        }
        RandomProjectionHDC {
            projection_vectors,
            input_dim,
        }
    }

    /// Project a slice of f64 values into HD space using weighted bundling.
    ///
    /// Each input feature `x_i` with value `v_i` contributes `floor(|v_i| * scale)`
    /// copies of its projection vector (or its negation, if negative) to the bundle.
    pub fn project(&self, features: &[f64], scale: f64) -> Hypervector {
        if features.is_empty() {
            return Hypervector::new_zero();
        }

        let n = features.len().min(self.input_dim);
        let mut weighted_vectors: Vec<Hypervector> = Vec::new();

        for i in 0..n {
            let val = features[i];
            let copies = ((val.abs() * scale).round() as usize).max(0).min(20);

            if copies == 0 {
                continue;
            }

            let base_vec = if val >= 0.0 {
                self.projection_vectors[i]
            } else {
                // Negation in binary HDC is the bitwise NOT (inverse)
                let mut negated = self.projection_vectors[i];
                for block in negated.bits.iter_mut() {
                    *block = !*block;
                }
                negated
            };

            for _ in 0..copies {
                weighted_vectors.push(base_vec);
            }
        }

        if weighted_vectors.is_empty() {
            return Hypervector::new_zero();
        }

        let refs: Vec<&Hypervector> = weighted_vectors.iter().collect();
        Hypervector::bundle(&refs)
    }
}

// ─── Visual Modality ─────────────────────────────────────────────────────

/// Encodes visual information into hypervectors using a random projection
/// of the HSV/grayscale histogram or downscaled pixel values.
///
/// For a real deployment, you would replace this with a small CNN feature
/// extractor (e.g., MobileNet) whose embedding layer output is projected
/// into the HD space. Here we use a simpler approach: color histogram +
/// spatial layout features.
pub struct VisualModality {
    pub name: String,
    /// Random projection from raw visual features → HD space
    projector: RandomProjectionHDC,
    /// Pixel data as grayscale values (0.0 - 1.0), flattened
    pixels: Vec<f64>,
    /// Image dimensions
    width: usize,
    height: usize,
}

impl VisualModality {
    /// Create a new visual modality with a given image size.
    pub fn new(name: &str, width: usize, height: usize) -> Self {
        // We project from: color histogram (64 bins) + spatial grid (8x8) + edge features
        let feature_dim = 64 + (width.min(8) * height.min(8)) + 16;
        VisualModality {
            name: name.to_string(),
            projector: RandomProjectionHDC::new(feature_dim),
            pixels: vec![0.0; width * height],
            width,
            height,
        }
    }

    /// Load pixel data from a flat grayscale array (0.0 - 1.0).
    pub fn load_pixels(&mut self, pixels: &[f64]) {
        let len = self.width * self.height;
        for (i, &p) in pixels.iter().enumerate().take(len) {
            self.pixels[i] = p.clamp(0.0, 1.0);
        }
    }

    /// Load pixel data from raw u8 grayscale values (0-255).
    pub fn load_pixels_u8(&mut self, pixels: &[u8]) {
        let len = self.width * self.height;
        for (i, &p) in pixels.iter().enumerate().take(len) {
            self.pixels[i] = p as f64 / 255.0;
        }
    }

    /// Encode the current visual into a hypervector.
    ///
    /// Extracts:
    /// 1. Color histogram (64 bins over the grayscale range)
    /// 2. Spatial layout (downscaled grid)
    /// 3. Edge features (horizontal/vertical gradients)
    fn extract_features(&self) -> Vec<f64> {
        let mut features = Vec::new();

        // 1. Grayscale histogram (64 bins)
        let mut histogram = vec![0.0; 64];
        for &p in &self.pixels {
            let bin = (p * 63.0).round() as usize;
            histogram[bin.min(63)] += 1.0;
        }
        let total = self.pixels.len() as f64;
        for h in histogram.iter_mut() {
            *h /= total;
        }
        features.extend(histogram);

        // 2. Spatial layout (downscale to 8x8 grid)
        let grid_w = self.width.min(8);
        let grid_h = self.height.min(8);
        let step_x = self.width / grid_w.max(1);
        let step_y = self.height / grid_h.max(1);

        for gy in 0..grid_h {
            for gx in 0..grid_w {
                let mut sum = 0.0;
                let mut count = 0;
                for y in gy * step_y..((gy + 1) * step_y).min(self.height) {
                    for x in gx * step_x..((gx + 1) * step_x).min(self.width) {
                        sum += self.pixels[y * self.width + x];
                        count += 1;
                    }
                }
                features.push(if count > 0 { sum / count as f64 } else { 0.0 });
            }
        }

        // 3. Simple edge features (horizontal & vertical gradients)
        let mut edge_features = vec![0.0; 16];
        for y in 1..self.height {
            for x in 1..self.width {
                let dx = self.pixels[y * self.width + x] - self.pixels[y * self.width + (x - 1)];
                let dy = self.pixels[y * self.width + x] - self.pixels[(y - 1) * self.width + x];
                let mag = (dx * dx + dy * dy).sqrt();
                let bin = (mag * 15.0).round() as usize;
                edge_features[bin.min(15)] += 1.0;
            }
        }
        let total_edges: f64 = edge_features.iter().sum();
        for e in edge_features.iter_mut() {
            *e /= total_edges.max(1.0);
        }
        features.extend(edge_features);

        features
    }
}

impl SensoryModality for VisualModality {
    fn encode(&self) -> Hypervector {
        let features = self.extract_features();
        self.projector.project(&features, 10.0)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ─── Audio Modality ─────────────────────────────────────────────────────

/// Encodes audio information into hypervectors using mel-spectrogram-like
/// features projected into HD space.
///
/// For a real deployment, replace with a proper mel-spectrogram + small
/// audio feature extractor. Here we use a simplified filterbank approach.
pub struct AudioModality {
    pub name: String,
    /// Random projection from audio features → HD space
    projector: RandomProjectionHDC,
    /// Raw audio samples (normalized -1.0 to 1.0)
    samples: Vec<f64>,
    /// Sample rate
    sample_rate: usize,
}

impl AudioModality {
    pub fn new(name: &str, sample_rate: usize) -> Self {
        // Feature dimensions: spectral bands (32) + temporal envelope (16) + zero-crossing rate
        let feature_dim = 32 + 16 + 4;
        AudioModality {
            name: name.to_string(),
            projector: RandomProjectionHDC::new(feature_dim),
            samples: Vec::new(),
            sample_rate,
        }
    }

    /// Load raw audio samples (normalized -1.0 to 1.0).
    pub fn load_samples(&mut self, samples: &[f64]) {
        self.samples = samples.to_vec();
    }

    /// Load audio from f32 samples.
    pub fn load_samples_f32(&mut self, samples: &[f32]) {
        self.samples = samples.iter().map(|&s| s as f64).collect();
    }

    /// Extract simple spectral and temporal features from the audio buffer.
    ///
    /// Uses a filterbank approach: splits the audio into frequency bands
    /// and computes energy in each band, plus temporal envelope features.
    fn extract_features(&self) -> Vec<f64> {
        let mut features = Vec::new();

        if self.samples.is_empty() {
            return vec![0.0; 32 + 16 + 4];
        }

        // 1. Spectral bands (32) via simplified FFT filterbank
        let frame_size = 256;
        let num_frames = (self.samples.len() / frame_size).max(1);
        let mut spectral_bands = vec![0.0; 32];

        for frame in 0..num_frames.min(100) {
            let start = frame * frame_size;
            let end = (start + frame_size).min(self.samples.len());
            let frame_samples = &self.samples[start..end];

            // Simple Goertzel-like filterbank: dot product with sinusoids
            for band in 0..32 {
                let freq = (band as f64 + 1.0) * self.sample_rate as f64 / (2.0 * 32.0);
                let omega = 2.0 * std::f64::consts::PI * freq / self.sample_rate as f64;
                let mut real = 0.0;
                let mut imag = 0.0;
                for (i, &sample) in frame_samples.iter().enumerate() {
                    let t = i as f64 * omega;
                    real += sample * t.cos();
                    imag += sample * t.sin();
                }
                spectral_bands[band] += (real * real + imag * imag).sqrt();
            }
        }

        let max_band = spectral_bands.iter().cloned().fold(0.0, f64::max);
        for b in spectral_bands.iter_mut() {
            *b /= max_band.max(1.0);
        }
        features.extend(spectral_bands.clone());

        // 2. Temporal envelope (16 segments of RMS energy)
        let segment_size = (self.samples.len() / 16).max(1);
        for seg in 0..16 {
            let start = seg * segment_size;
            let end = (start + segment_size).min(self.samples.len());
            let segment = &self.samples[start..end];
            let rms = (segment.iter().map(|&s| s * s).sum::<f64>() / segment.len() as f64).sqrt();
            features.push(rms);
        }

        // 3. Zero-crossing rate and other simple features
        let mut zcr = 0.0;
        for i in 1..self.samples.len() {
            if self.samples[i] * self.samples[i - 1] < 0.0 {
                zcr += 1.0;
            }
        }
        zcr /= self.samples.len() as f64;
        features.push(zcr);

        let energy = self.samples.iter().map(|&s| s * s).sum::<f64>() / self.samples.len() as f64;
        features.push(energy);

        let mean = self.samples.iter().sum::<f64>() / self.samples.len() as f64;
        features.push(mean.abs());

        // Spectral centroid estimate
        let mut centroid_num = 0.0;
        let mut centroid_den = 0.0;
        for (i, &band) in spectral_bands.iter().enumerate() {
            centroid_num += (i as f64) * band;
            centroid_den += band;
        }
        features.push(if centroid_den > 0.0 {
            centroid_num / centroid_den / 31.0
        } else {
            0.0
        });

        features
    }
}

impl SensoryModality for AudioModality {
    fn encode(&self) -> Hypervector {
        let features = self.extract_features();
        self.projector.project(&features, 15.0)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ─── Unified Latent Space ────────────────────────────────────────────────

/// The Unified Latent Space mapper cross-modal concept alignment.
///
/// By mapping similar concepts from different modalities to similar hypervectors,
/// the machine achieves inherent multimodal understanding.
///
/// For example:
/// - The image of a cat → H_cat_image
/// - The word "cat" → H_cat_text
/// - These should have high normalized Hamming similarity (>0.75)
///
/// This module maintains a set of cross-modal alignment mappings.
pub struct UnifiedLatentSpace {
    /// Cross-modal concept mappings: concept_name → aligned hypervector
    concepts: HashMap<String, UnifiedConcept>,
    /// HNSW index for O(log n) cross-modal concept retrieval.
    /// Maps canonical hypervectors to concept indices.
    hnsw_index: Option<crate::hnsw::HnswIndex>,
    /// Maps HNSW index → concept name
    hnsw_to_concept: Vec<String>,
}

/// A concept that exists across multiple modalities.
#[derive(Clone, Debug)]
pub struct UnifiedConcept {
    /// The canonical aligned hypervector for this concept
    pub canonical: Hypervector,
    /// Text modality vector (from n-gram encoding)
    pub text_vector: Hypervector,
    /// Visual modality vector (projected from image features)
    pub visual_vector: Option<Hypervector>,
    /// Audio modality vector (projected from audio features)
    pub audio_vector: Option<Hypervector>,
    /// Human-readable label
    pub label: String,
}

impl UnifiedLatentSpace {
    pub fn new() -> Self {
        UnifiedLatentSpace {
            concepts: HashMap::new(),
            hnsw_index: None,
            hnsw_to_concept: Vec::new(),
        }
    }

    /// Register a new cross-modal concept.
    /// `canonical` should be the aligned hypervector (same for all modalities).
    pub fn register_concept(
        &mut self,
        label: &str,
        text_vector: Hypervector,
        visual_vector: Option<Hypervector>,
        audio_vector: Option<Hypervector>,
    ) -> Hypervector {
        // The canonical vector is derived from the text vector but aligned
        // to be close to all modality-specific representations
        let mut alignment = text_vector;

        if let Some(ref vis) = visual_vector {
            // Blend visual info into the canonical representation
            alignment = Hypervector::bundle(&[&alignment, vis]);
        }
        if let Some(ref aud) = audio_vector {
            alignment = Hypervector::bundle(&[&alignment, aud]);
        }

        let concept = UnifiedConcept {
            canonical: alignment,
            text_vector,
            visual_vector,
            audio_vector,
            label: label.to_string(),
        };

        self.concepts.insert(label.to_string(), concept);
        alignment
    }

    /// Get the canonical hypervector for a registered concept.
    pub fn get_canonical(&self, label: &str) -> Option<&Hypervector> {
        self.concepts.get(label).map(|c| &c.canonical)
    }

    /// Get the text vector for a concept.
    pub fn get_text_vector(&self, label: &str) -> Option<&Hypervector> {
        self.concepts.get(label).map(|c| &c.text_vector)
    }

    /// Rebuild the HNSW index for accelerated cross-modal query.
    pub fn rebuild_hnsw_index(&mut self) {
        if self.concepts.is_empty() {
            self.hnsw_index = None;
            self.hnsw_to_concept = Vec::new();
            return;
        }

        let mut index = crate::hnsw::HnswIndex::with_config(crate::hnsw::HnswConfig {
            use_heuristic: true,
            ..crate::hnsw::HnswConfig::default()
        });

        let mut mapping = Vec::new();
        for (label, concept) in &self.concepts {
            let _idx = index.insert(&concept.canonical.bits);
            mapping.push(label.clone());
        }

        self.hnsw_index = Some(index);
        self.hnsw_to_concept = mapping;
    }

    /// Ensure the HNSW index is fresh.
    pub fn ensure_hnsw_index(&mut self) {
        let needs_rebuild = match self.hnsw_index {
            Some(_) => self.hnsw_to_concept.len() != self.concepts.len(),
            None => !self.concepts.is_empty(),
        };
        if needs_rebuild {
            self.rebuild_hnsw_index();
        }
    }

    /// Find the closest registered concept to a query hypervector (linear scan).
    pub fn query(&self, query: &Hypervector, threshold: f64) -> Vec<(String, f64, String)> {
        let mut results = Vec::new();
        for (label, concept) in &self.concepts {
            let sim = 1.0 - query.normalized_hamming_distance(&concept.canonical);
            if sim >= threshold {
                let modality = if concept.visual_vector.is_some() && concept.audio_vector.is_some()
                {
                    "multimodal"
                } else if concept.visual_vector.is_some() {
                    "visual"
                } else if concept.audio_vector.is_some() {
                    "audio"
                } else {
                    "text"
                };
                results.push((label.clone(), sim, modality.to_string()));
            }
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// HNSW-accelerated query. Requires &mut self for lazy index rebuild.
    /// For large concept sets, this is O(log n) instead of O(n).
    pub fn query_hnsw(
        &mut self,
        query: &Hypervector,
        threshold: f64,
        top_k: usize,
    ) -> Vec<(String, f64, String)> {
        self.ensure_hnsw_index();

        if let Some(ref index) = self.hnsw_index {
            let result = index.search_by_hypervector(query, top_k * 2);
            let mut output = Vec::new();

            for (i, dist) in result.indices.iter().zip(result.distances.iter()) {
                if *i >= self.hnsw_to_concept.len() {
                    continue;
                }
                let sim = 1.0 - dist;
                if sim >= threshold {
                    let label = &self.hnsw_to_concept[*i];
                    if let Some(concept) = self.concepts.get(label) {
                        let modality =
                            if concept.visual_vector.is_some() && concept.audio_vector.is_some() {
                                "multimodal"
                            } else if concept.visual_vector.is_some() {
                                "visual"
                            } else if concept.audio_vector.is_some() {
                                "audio"
                            } else {
                                "text"
                            };
                        output.push((label.clone(), sim, modality.to_string()));
                    }
                }
            }
            output.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            output.truncate(top_k);
            return output;
        }

        // Fallback
        self.query(query, threshold)
    }

    /// Align a text vector to the unified space by blending it with similar
    /// existing concepts. This enables zero-shot cross-modal understanding.
    pub fn align_text(&self, text_vector: &Hypervector) -> Hypervector {
        let threshold = 0.55;
        let mut blend_vectors = vec![text_vector];

        for concept in self.concepts.values() {
            let sim = 1.0 - text_vector.normalized_hamming_distance(&concept.canonical);
            if sim >= threshold {
                blend_vectors.push(&concept.canonical);
            }
        }

        let refs: Vec<&Hypervector> = blend_vectors.iter().map(|v| *v).collect();
        Hypervector::bundle(&refs)
    }

    /// Number of registered concepts.
    pub fn len(&self) -> usize {
        self.concepts.len()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_projection() {
        let projector = RandomProjectionHDC::new(10);
        let features1 = vec![0.5, 0.3, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let features2 = vec![0.5, 0.3, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // same
        let features3 = vec![1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1]; // different

        let hv1 = projector.project(&features1, 5.0);
        let hv2 = projector.project(&features2, 5.0);
        let hv3 = projector.project(&features3, 5.0);

        // Same features should produce same vector
        assert_eq!(hv1, hv2);

        // Different features should produce different vectors
        let dist = hv1.normalized_hamming_distance(&hv3);
        assert!(
            dist > 0.10,
            "Different features should produce distinguishable vectors: {}",
            dist
        );
    }

    #[test]
    fn test_visual_modality() {
        let mut visual = VisualModality::new("camera", 32, 32);
        // Create a simple pattern: white square on black background
        let mut pixels = vec![0.0; 32 * 32];
        for y in 8..24 {
            for x in 8..24 {
                pixels[y * 32 + x] = 1.0;
            }
        }
        visual.load_pixels(&pixels);
        let hv = visual.encode();

        // Should not be zero
        let zero = Hypervector::new_zero();
        assert_ne!(hv, zero);

        // Consistent encoding
        let hv2 = visual.encode();
        assert_eq!(hv, hv2);
    }

    #[test]
    fn test_audio_modality() {
        let mut audio = AudioModality::new("mic", 44100);
        // Generate a simple sine wave
        let mut samples = Vec::new();
        let freq = 440.0; // A4 note
        for i in 0..4410 {
            let t = i as f64 / 44100.0;
            samples.push((2.0 * std::f64::consts::PI * freq * t).sin());
        }
        audio.load_samples(&samples);
        let hv = audio.encode();

        // Should not be zero
        let zero = Hypervector::new_zero();
        assert_ne!(hv, zero);

        // Consistent encoding
        let hv2 = audio.encode();
        assert_eq!(hv, hv2);

        // Different frequency should produce different vector
        let mut audio2 = AudioModality::new("mic2", 44100);
        let mut samples2 = Vec::new();
        let freq2 = 880.0; // A5 note
        for i in 0..4410 {
            let t = i as f64 / 44100.0;
            samples2.push((2.0 * std::f64::consts::PI * freq2 * t).sin());
        }
        audio2.load_samples(&samples2);
        let hv2 = audio2.encode();

        let dist = hv.normalized_hamming_distance(&hv2);
        assert!(
            dist > 0.10,
            "Different frequencies should be distinguishable: {}",
            dist
        );
    }

    #[test]
    fn test_unified_latent_space() {
        let mut uls = UnifiedLatentSpace::new();

        // Register a text-only concept
        let cat_text = Hypervector::encode_sentence("a cute cat");
        uls.register_concept("cat", cat_text, None, None);

        // Register a multimodal concept
        let dog_text = Hypervector::encode_sentence("a friendly dog");
        let dog_visual = Hypervector::encode_text_ngram("dog_image_features", 3);
        let dog_audio = Hypervector::encode_text_ngram("dog_bark_features", 3);
        uls.register_concept("dog", dog_text, Some(dog_visual), Some(dog_audio));

        // Query for dog
        let results = uls.query(&dog_text, 0.50);
        assert!(!results.is_empty(), "Should find dog concept");
        assert!(results.iter().any(|(label, _, _)| label == "dog"));

        // Verify multimodal tagging
        let dog_result = results.iter().find(|(l, _, _)| l == "dog").unwrap();
        assert_eq!(dog_result.2, "multimodal");
    }

    #[test]
    fn test_cross_modal_alignment() {
        let mut uls = UnifiedLatentSpace::new();

        // Register a concept with aligned representations
        let text_hv = Hypervector::encode_sentence("a red apple");
        let visual_hv = Hypervector::encode_text_ngram("red_round_fruit_features", 3);
        uls.register_concept("apple", text_hv, Some(visual_hv), None);

        // A closely related text should align toward the visual representation
        let similar_text = Hypervector::encode_sentence("red apple fruit");
        let aligned = uls.align_text(&similar_text);

        // The aligned vector should be closer to the canonical apple concept
        let canonical = uls.get_canonical("apple").unwrap();
        let sim_before = 1.0 - similar_text.normalized_hamming_distance(canonical);
        let sim_after = 1.0 - aligned.normalized_hamming_distance(canonical);
        assert!(
            sim_after >= sim_before - 0.05,
            "Alignment should maintain or improve similarity: before={}, after={}",
            sim_before,
            sim_after
        );
    }
}
