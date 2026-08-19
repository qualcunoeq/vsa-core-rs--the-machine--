//! Stage 380: freeze the source inventory for a broader external curriculum.
//!
//! This stage does not extract exercises or answer keys.  It records immutable
//! file hashes and provenance for the locally pinned OpenStax and LibreTexts
//! PDFs so later extraction can be audited against the exact source release.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const JSON: &str = "docs/stage380_external_curriculum_source_manifest.json";
const MD: &str = "docs/stage380_external_curriculum_source_manifest.md";

#[derive(Debug, Serialize)]
struct SourceFile {
    path: String,
    family: String,
    bytes: u64,
    sha256: String,
    extraction_status: &'static str,
    answer_key_status: &'static str,
}

#[derive(Debug, Serialize)]
struct Manifest {
    schema: &'static str,
    source_roots: Vec<String>,
    files: Vec<SourceFile>,
    file_count: usize,
    extraction_status: &'static str,
    answer_key_policy: &'static str,
    manifest_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn collect_pdfs(root: &Path, output: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_pdfs(&path, output)?;
        } else if path.extension().is_some_and(|extension| extension == "pdf") {
            output.push(path);
        }
    }
    Ok(())
}

fn unsigned_payload(manifest: &Manifest) -> impl Serialize + '_ {
    (
        manifest.schema,
        &manifest.source_roots,
        &manifest.files,
        manifest.file_count,
        manifest.extraction_status,
        manifest.answer_key_policy,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let roots = [
        Path::new("data/openstax_pdfs"),
        Path::new("data/libretexts_pdfs"),
    ];
    let mut paths = Vec::new();
    for root in roots {
        collect_pdfs(root, &mut paths)?;
    }
    paths.sort();
    assert!(
        !paths.is_empty(),
        "the pinned external source inventory is empty"
    );
    let files = paths
        .iter()
        .map(|path| {
            let bytes = fs::read(path).expect("source PDF is readable");
            let family = if path.to_string_lossy().contains("openstax") {
                "openstax"
            } else {
                "libretexts"
            };
            SourceFile {
                path: path.to_string_lossy().into_owned(),
                family: family.into(),
                bytes: bytes.len() as u64,
                sha256: digest_bytes(&bytes),
                extraction_status: "not_attempted",
                answer_key_status: "not_inspected",
            }
        })
        .collect::<Vec<_>>();
    let mut manifest = Manifest {
        schema: "stage380-external-curriculum-source-manifest-v1",
        source_roots: vec!["data/openstax_pdfs".into(), "data/libretexts_pdfs".into()],
        file_count: files.len(),
        files,
        extraction_status: "not_attempted",
        answer_key_policy: "no answer keys are read or inferred during inventory",
        manifest_sha256: String::new(),
    };
    let manifest_sha256 = digest_bytes(
        serde_json::to_vec(&unsigned_payload(&manifest))
            .expect("source manifest serializes")
            .as_slice(),
    );
    manifest.manifest_sha256 = manifest_sha256;
    assert_eq!(manifest.file_count, manifest.files.len());
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    let openstax = manifest
        .files
        .iter()
        .filter(|file| file.family == "openstax")
        .count();
    let libretexts = manifest
        .files
        .iter()
        .filter(|file| file.family == "libretexts")
        .count();
    fs::write(
        MD,
        format!(
            "# Stage 380 — external curriculum source manifest\n\n- pinned PDF files: {} (OpenStax {}, LibreTexts {})\n- extraction status: `{}`\n- answer-key policy: {}\n- manifest SHA-256: `{}`\n\nThis is an immutable inventory boundary for the planned broader naturally authored curriculum. It records file hashes and provenance only; no PDF text, exercise, solution, or answer key was extracted or inspected in this stage. A later extraction campaign must consume this manifest and refuse hash drift.\n\nReproduce with `cargo run --quiet --bin stage380_external_curriculum_source_manifest`.\nMachine-readable manifest: `{}`\n",
            manifest.file_count,
            openstax,
            libretexts,
            manifest.extraction_status,
            manifest.answer_key_policy,
            manifest.manifest_sha256,
            JSON,
        ),
    )?;
    println!(
        "stage380 files={} openstax={} libretexts={} extraction={} manifest_sha256={}",
        manifest.file_count,
        openstax,
        libretexts,
        manifest.extraction_status,
        manifest.manifest_sha256,
    );
    Ok(())
}
