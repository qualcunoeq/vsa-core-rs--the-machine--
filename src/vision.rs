//! Deterministic visual perception and visual memory.
//!
//! `VisionSystem` decodes ordinary image files, extracts a compact descriptor
//! (colour, luminance layout, and edge density), and can retrieve the nearest
//! labelled visual memory. It deliberately does not pretend to be a general
//! object detector: semantic labels come from examples supplied to
//! `remember_path`/`remember_rgb`, making every recognition result auditable.

use crate::perception::{Entity, PerceptualEncoder, SvoTriple};
use crate::{Hypervector, HD_DIMENSION};
use image::{DynamicImage, GenericImageView};
use std::path::Path;

#[path = "visual_table.rs"]
pub mod visual_table;

#[path = "visual_graph.rs"]
pub mod visual_graph;

#[path = "visual_source_statistics_bridge.rs"]
pub mod visual_source_statistics_bridge;

const COLOR_BINS: usize = 4;
const GRID: usize = 4;
const DESCRIPTOR_DIM: usize = COLOR_BINS * COLOR_BINS * COLOR_BINS + GRID * GRID + 4;

/// A word emitted by OCR, retaining its image coordinates.  Text without
/// coordinates is not enough to distinguish an axis label from a table cell
/// or to establish even simple diagram relationships.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OcrWord {
    pub text: String,
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

/// Conservative, inspectable structure recovered from an OCR TSV stream.
/// These fields are observations, not semantic claims about the diagram.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuredDiagram {
    pub text: String,
    pub labels: Vec<String>,
    pub horizontal_axis_labels: Vec<String>,
    pub vertical_axis_labels: Vec<String>,
    pub table_cells: Vec<Vec<String>>,
    pub relationships: Vec<String>,
}

impl StructuredDiagram {
    /// Parse Tesseract's TSV output and recover only coordinate-grounded
    /// layout. Bad rows are skipped; no guessed text or graph values are
    /// introduced.  This makes the result suitable as a routing input or an
    /// auditable answer-choice constraint, not as a general diagram solver.
    pub fn from_tesseract_tsv(tsv: &str, image_width: u32, image_height: u32) -> Self {
        let words = parse_tesseract_tsv(tsv);
        let text = words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let mut labels: Vec<String> = words.iter().map(|word| word.text.clone()).collect();
        labels.sort();
        labels.dedup();

        let horizontal_axis_labels = words
            .iter()
            .filter(|word| word.top.saturating_mul(100) >= image_height.saturating_mul(85))
            .map(|word| word.text.clone())
            .collect();
        let vertical_axis_labels = words
            .iter()
            .filter(|word| word.left.saturating_add(word.width).saturating_mul(4) <= image_width)
            .map(|word| word.text.clone())
            .collect();
        let table_cells = group_words_into_rows(&words);
        let relationships = spatial_relationships(&words);
        Self {
            text,
            labels,
            horizontal_axis_labels,
            vertical_axis_labels,
            table_cells,
            relationships,
        }
    }
}

/// Parse the word-level rows (level 5) of Tesseract TSV output.
pub fn parse_tesseract_tsv(tsv: &str) -> Vec<OcrWord> {
    tsv.lines()
        .skip_while(|line| line.starts_with("level\t"))
        .filter_map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() < 12 || fields[0] != "5" {
                return None;
            }
            let text = fields[11].trim();
            if text.is_empty() {
                return None;
            }
            Some(OcrWord {
                text: text.to_string(),
                left: fields[6].parse().ok()?,
                top: fields[7].parse().ok()?,
                width: fields[8].parse().ok()?,
                height: fields[9].parse().ok()?,
            })
        })
        .collect()
}

fn group_words_into_rows(words: &[OcrWord]) -> Vec<Vec<String>> {
    let mut ordered = words.to_vec();
    ordered.sort_by_key(|word| (word.top, word.left));
    let mut rows: Vec<(u32, Vec<OcrWord>)> = Vec::new();
    for word in ordered {
        let tolerance = word.height.max(8);
        if let Some((top, row)) = rows
            .iter_mut()
            .find(|(top, _)| word.top.abs_diff(*top) <= tolerance)
        {
            *top = (*top).min(word.top);
            row.push(word);
        } else {
            rows.push((word.top, vec![word]));
        }
    }
    rows.sort_by_key(|(top, _)| *top);
    rows.into_iter()
        .map(|(_, mut row)| {
            row.sort_by_key(|word| word.left);
            row.into_iter().map(|word| word.text).collect()
        })
        .collect()
}

fn spatial_relationships(words: &[OcrWord]) -> Vec<String> {
    // A small cap prevents a dense OCR page from becoming a quadratic trace.
    let words = &words[..words.len().min(64)];
    let mut relations = Vec::new();
    for (index, left) in words.iter().enumerate() {
        for right in words.iter().skip(index + 1) {
            let left_center = left.left + left.width / 2;
            let right_center = right.left + right.width / 2;
            let top_center = left.top + left.height / 2;
            let bottom_center = right.top + right.height / 2;
            if left_center.saturating_add(left.width.max(right.width)) < right_center {
                relations.push(format!("{} is left of {}", left.text, right.text));
            }
            if top_center.saturating_add(left.height.max(right.height)) < bottom_center {
                relations.push(format!("{} is above {}", left.text, right.text));
            }
            if relations.len() >= 32 {
                return relations;
            }
        }
    }
    relations
}

#[derive(Clone, Debug)]
pub struct VisualObservation {
    pub width: u32,
    pub height: u32,
    /// Normalised descriptor: RGB histogram, spatial luminance grid, summary.
    pub descriptor: Vec<f32>,
    pub embedding: Hypervector,
}

impl VisualObservation {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let image = image::open(path)
            .map_err(|error| format!("could not decode image '{}': {error}", path.display()))?;
        Ok(Self::from_image(&image))
    }

    pub fn from_rgb(width: u32, height: u32, pixels: &[u8]) -> Result<Self, String> {
        let expected = width as usize * height as usize * 3;
        if width == 0 || height == 0 || pixels.len() != expected {
            return Err(format!(
                "RGB image needs exactly {expected} bytes for {width}x{height}, got {}",
                pixels.len()
            ));
        }
        let image = image::RgbImage::from_raw(width, height, pixels.to_vec())
            .ok_or_else(|| "invalid RGB image buffer".to_string())?;
        Ok(Self::from_image(&DynamicImage::ImageRgb8(image)))
    }

    pub fn similarity(&self, other: &Self) -> f32 {
        cosine_similarity(&self.descriptor, &other.descriptor)
    }

    fn from_image(image: &DynamicImage) -> Self {
        let rgb = image.to_rgb8();
        let (width, height) = image.dimensions();
        let descriptor = extract_descriptor(width, height, rgb.as_raw());
        let embedding = descriptor_embedding(&descriptor);
        Self {
            width,
            height,
            descriptor,
            embedding,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VisualMatch {
    pub label: String,
    pub similarity: f32,
}

#[derive(Clone, Debug)]
struct VisualMemory {
    label: String,
    observation: VisualObservation,
}

/// A local, example-based visual recognizer.
#[derive(Default)]
pub struct VisionSystem {
    memories: Vec<VisualMemory>,
}

impl VisionSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.memories.len()
    }

    pub fn remember_path(
        &mut self,
        label: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<(), String> {
        self.remember(label, VisualObservation::from_path(path)?);
        Ok(())
    }

    pub fn remember_rgb(
        &mut self,
        label: impl Into<String>,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<(), String> {
        self.remember(label, VisualObservation::from_rgb(width, height, pixels)?);
        Ok(())
    }

    pub fn remember(&mut self, label: impl Into<String>, observation: VisualObservation) {
        self.memories.push(VisualMemory {
            label: label.into(),
            observation,
        });
    }

    pub fn observe_path(&self, path: impl AsRef<Path>) -> Result<VisualObservation, String> {
        VisualObservation::from_path(path)
    }

    pub fn recognize(
        &self,
        observation: &VisualObservation,
        min_similarity: f32,
    ) -> Option<VisualMatch> {
        self.memories
            .iter()
            .map(|memory| VisualMatch {
                label: memory.label.clone(),
                similarity: observation.similarity(&memory.observation),
            })
            .filter(|candidate| candidate.similarity >= min_similarity)
            .max_by(|a, b| {
                a.similarity
                    .partial_cmp(&b.similarity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    pub fn recognize_path(
        &self,
        path: impl AsRef<Path>,
        min_similarity: f32,
    ) -> Result<Option<VisualMatch>, String> {
        Ok(self.recognize(&self.observe_path(path)?, min_similarity))
    }

    /// Convert a labelled observation into the Machine's native SVO form.
    /// The label is asserted by the caller/training example, not hallucinated.
    pub fn scene_triples(&self, label: &str, observation: &VisualObservation) -> Vec<SvoTriple> {
        let brightness = observation.descriptor[COLOR_BINS.pow(3) + GRID * GRID];
        let edges = observation.descriptor[COLOR_BINS.pow(3) + GRID * GRID + 1];
        vec![
            (
                label.to_string(),
                "has_visual_property".to_string(),
                brightness_name(brightness).to_string(),
            ),
            (
                label.to_string(),
                "has_edge_density".to_string(),
                edge_name(edges).to_string(),
            ),
            (
                label.to_string(),
                "has_aspect_ratio".to_string(),
                aspect_name(observation.width, observation.height).to_string(),
            ),
        ]
    }
}

/// Adapter for callers that want image descriptors in the existing perceptual
/// pipeline. It emits only grounded, measurable relations.
pub struct VisionEncoder;

impl PerceptualEncoder for VisionEncoder {
    type Input = (String, VisualObservation);

    fn extract_entities(&self, input: &Self::Input) -> Vec<Entity> {
        vec![input.0.clone()]
    }

    fn extract_relations(&self, input: &Self::Input, _entities: &[Entity]) -> Vec<SvoTriple> {
        VisionSystem::new().scene_triples(&input.0, &input.1)
    }
}

fn extract_descriptor(width: u32, height: u32, pixels: &[u8]) -> Vec<f32> {
    let mut histogram = vec![0.0f32; COLOR_BINS.pow(3)];
    let mut grid_sum = [0.0f32; GRID * GRID];
    let mut grid_count = [0u32; GRID * GRID];
    let mut luminance_sum = 0.0f32;
    let mut edge_sum = 0.0f32;
    let mut pixel_count = 0u32;
    let luminance = |x: u32, y: u32| -> f32 {
        let i = ((y * width + x) * 3) as usize;
        (0.2126 * pixels[i] as f32 + 0.7152 * pixels[i + 1] as f32 + 0.0722 * pixels[i + 2] as f32)
            / 255.0
    };

    for y in 0..height {
        for x in 0..width {
            let i = ((y * width + x) * 3) as usize;
            let r = pixels[i] as usize * COLOR_BINS / 256;
            let g = pixels[i + 1] as usize * COLOR_BINS / 256;
            let b = pixels[i + 2] as usize * COLOR_BINS / 256;
            histogram[(r * COLOR_BINS + g) * COLOR_BINS + b] += 1.0;
            let l = luminance(x, y);
            luminance_sum += l;
            let gx = (x as usize * GRID / width as usize).min(GRID - 1);
            let gy = (y as usize * GRID / height as usize).min(GRID - 1);
            grid_sum[gy * GRID + gx] += l;
            grid_count[gy * GRID + gx] += 1;
            if x > 0 && y > 0 {
                edge_sum += (l - luminance(x - 1, y)).abs() + (l - luminance(x, y - 1)).abs();
            }
            pixel_count += 1;
        }
    }
    let count = pixel_count.max(1) as f32;
    for bin in &mut histogram {
        *bin /= count;
    }
    let grid: Vec<f32> = grid_sum
        .iter()
        .zip(grid_count)
        .map(|(sum, count)| *sum / count.max(1) as f32)
        .collect();
    let mean = luminance_sum / count;
    let edge_density = edge_sum / count;
    let aspect = (width as f32 / height.max(1) as f32).ln().clamp(-2.0, 2.0) / 2.0;
    let mut output = histogram;
    output.extend(grid);
    output.extend([mean, edge_density, aspect, 1.0]);
    debug_assert_eq!(output.len(), DESCRIPTOR_DIM);
    output
}

fn descriptor_embedding(descriptor: &[f32]) -> Hypervector {
    // Deterministic signed random projection derived from dimension and value.
    // It is stable across processes, unlike an unseeded random projection.
    let mut bits = [0u64; crate::U64_BLOCKS];
    for bit in 0..HD_DIMENSION {
        let mut sum = 0.0f32;
        for (dimension, value) in descriptor.iter().enumerate() {
            let hash = (bit as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (dimension as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            sum += if (hash ^ (hash >> 30)).count_ones() & 1 == 0 {
                *value
            } else {
                -*value
            };
        }
        if sum >= 0.0 {
            bits[bit / 64] |= 1u64 << (bit % 64);
        }
    }
    Hypervector { bits }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut a_norm, mut b_norm) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        a_norm += x * x;
        b_norm += y * y;
    }
    dot / (a_norm.sqrt() * b_norm.sqrt()).max(f32::EPSILON)
}

fn brightness_name(value: f32) -> &'static str {
    if value < 0.33 {
        "dark"
    } else if value > 0.67 {
        "bright"
    } else {
        "mid_tone"
    }
}
fn edge_name(value: f32) -> &'static str {
    if value < 0.08 {
        "low"
    } else if value > 0.25 {
        "high"
    } else {
        "medium"
    }
}
fn aspect_name(width: u32, height: u32) -> &'static str {
    if width > height {
        "landscape"
    } else if height > width {
        "portrait"
    } else {
        "square"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        rgb.into_iter()
            .cycle()
            .take(width as usize * height as usize * 3)
            .collect()
    }

    #[test]
    fn same_pixels_produce_stable_observation_and_embedding() {
        let pixels = solid(8, 8, [255, 0, 0]);
        let first = VisualObservation::from_rgb(8, 8, &pixels).unwrap();
        let second = VisualObservation::from_rgb(8, 8, &pixels).unwrap();
        assert_eq!(first.descriptor, second.descriptor);
        assert_eq!(first.embedding, second.embedding);
        assert!((first.similarity(&second) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn visual_memory_recognizes_a_label_and_rejects_a_dissimilar_image() {
        let red = solid(12, 12, [255, 0, 0]);
        let blue = solid(12, 12, [0, 0, 255]);
        let mut vision = VisionSystem::new();
        vision.remember_rgb("red_card", 12, 12, &red).unwrap();
        let observation = VisualObservation::from_rgb(12, 12, &red).unwrap();
        assert_eq!(
            vision.recognize(&observation, 0.99).unwrap().label,
            "red_card"
        );
        let blue_observation = VisualObservation::from_rgb(12, 12, &blue).unwrap();
        assert!(vision.recognize(&blue_observation, 0.99).is_none());
    }

    #[test]
    fn vision_encoder_emits_grounded_scene_triples() {
        let image = VisualObservation::from_rgb(4, 8, &solid(4, 8, [255, 255, 255])).unwrap();
        let triples = VisionEncoder.encode(&("page".to_string(), image));
        assert!(triples
            .iter()
            .any(|(_, relation, object)| relation == "has_visual_property" && object == "bright"));
        assert!(triples
            .iter()
            .any(|(_, relation, object)| relation == "has_aspect_ratio" && object == "portrait"));
    }

    #[test]
    fn malformed_rgb_is_rejected() {
        assert!(VisualObservation::from_rgb(2, 2, &[0; 11]).is_err());
    }

    #[test]
    fn tsv_extraction_preserves_labels_axes_cells_and_geometry() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
5\t1\t1\t1\t1\t1\t5\t80\t10\t10\t95\ty\n\
5\t1\t1\t1\t2\t1\t20\t20\t20\t10\t95\tMass\n\
5\t1\t1\t1\t2\t2\t70\t20\t10\t10\t95\tForce\n\
5\t1\t1\t1\t3\t1\t20\t55\t10\t10\t95\t2\n\
5\t1\t1\t1\t3\t2\t70\t55\t10\t10\t95\t10\n\
5\t1\t1\t1\t4\t1\t50\t92\t10\t8\t95\tx";
        let diagram = StructuredDiagram::from_tesseract_tsv(tsv, 100, 100);
        assert_eq!(diagram.text, "y Mass Force 2 10 x");
        assert!(diagram.labels.contains(&"Mass".to_string()));
        assert_eq!(diagram.horizontal_axis_labels, vec!["x"]);
        assert_eq!(diagram.vertical_axis_labels, vec!["y"]);
        assert!(diagram
            .table_cells
            .iter()
            .any(|row| row == &vec!["Mass".to_string(), "Force".to_string()]));
        assert!(diagram
            .relationships
            .iter()
            .any(|relation| relation == "Mass is left of Force"));
    }
}
