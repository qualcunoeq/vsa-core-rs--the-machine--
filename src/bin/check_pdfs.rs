//! Quick check of LibreTexts PDFs

fn main() {
    let dir = "data/libretexts_pdfs";
    let entries = std::fs::read_dir(dir).expect("Can't read dir");
    let mut pdfs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "pdf").unwrap_or(false))
        .collect();
    pdfs.sort_by_key(|e| e.path());

    for entry in &pdfs {
        let path = entry.path();
        let bytes = std::fs::read(&path).expect("Can't read file");
        let size_kb = bytes.len() / 1024;

        match pdf_extract::extract_text_from_mem(&bytes) {
            Ok(text) => {
                let cleaned: String = text
                    .chars()
                    .take(500)
                    .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
                    .collect();
                let first_line = cleaned
                    .lines()
                    .find(|l| l.trim().len() > 20)
                    .unwrap_or("(no content)")
                    .trim()
                    .to_string();
                println!(
                    "{} ({}KB): {}",
                    path.file_name().unwrap().to_string_lossy(),
                    size_kb,
                    first_line.chars().take(120).collect::<String>()
                );
            }
            Err(e) => {
                println!(
                    "{} ({}KB): EXTRACT ERROR: {}",
                    path.file_name().unwrap().to_string_lossy(),
                    size_kb,
                    e
                );
            }
        }
    }
}
