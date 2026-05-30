use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::BufReader;
use std::path::PathBuf;

// Extension preference when multiple files share the same stem (e.g. ARW + HIF pairs).
// Earlier in the list wins.
const PREFERRED_EXTS: &[&str] = &["arw", "jpg", "jpeg", "tif", "tiff", "hif", "heic"];

fn main() {
    let dir_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/Path/To/Images/".to_string());

    let entries = match fs::read_dir(&dir_path) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading directory '{}': {}", dir_path, e);
            return;
        }
    };

    // Deduplicate by stem: keep only the highest-priority extension per base name.
    let mut by_stem: HashMap<String, PathBuf> = HashMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let new_rank = PREFERRED_EXTS.iter().position(|&e| e == ext);
        let cur_rank = by_stem.get(&stem).and_then(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .and_then(|e| PREFERRED_EXTS.iter().position(|&x| x == e))
        });

        match (new_rank, cur_rank) {
            (Some(n), Some(c)) if n < c => { by_stem.insert(stem, path); }
            (Some(_), None) => { by_stem.insert(stem, path); }
            (None, None) => { by_stem.insert(stem, path); }
            _ => {}
        }
    }

    let mut paths: Vec<PathBuf> = by_stem.into_values().collect();
    paths.sort();

    let mut focal_lengths: Vec<String> = Vec::new();

    for path in &paths {
        println!("Path: {}", path.display());

        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("ERROR: {}", e);
                continue;
            }
        };

        let exif = match exif::Reader::new().read_from_container(&mut BufReader::new(file)) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("ERROR: {}", e);
                continue;
            }
        };

        for field in exif.fields() {
            if field.tag == exif::Tag::FocalLength {
                let desc = field.display_value().to_string();
                println!("Focal Length: {}", desc);
                focal_lengths.push(desc);
            }
        }
    }

    focal_lengths.sort();

    println!("\nFrequency Counter:");
    let mut focal_count: HashMap<String, u32> = HashMap::new();
    for fl in &focal_lengths {
        *focal_count.entry(fl.clone()).or_insert(0) += 1;
    }

    println!("{:?}", focal_count);
}
