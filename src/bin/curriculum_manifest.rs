//! Emit the governed breadth-first curriculum manifest.

use serde::Serialize;
use the_machine::curriculum::breadth_first_manifest;

#[derive(Serialize)]
struct Output {
    manifest: the_machine::curriculum::CurriculumManifest,
    manifest_replay_hash: String,
    validation_errors: Vec<String>,
    production_authorizations: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let output = Output {
        manifest_replay_hash: manifest.replay_hash(),
        validation_errors: manifest.validate(),
        manifest,
        production_authorizations: 0,
    };
    assert!(output.validation_errors.is_empty());
    assert_eq!(output.production_authorizations, 0);
    println!("{}", serde_json::to_string_pretty(&output)?);
    std::fs::write(
        "docs/curriculum_manifest.json",
        serde_json::to_vec_pretty(&output)?,
    )?;
    Ok(())
}
