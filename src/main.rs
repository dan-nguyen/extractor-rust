use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufReader, Cursor, Read};
use std::path::PathBuf;
use indicatif::{ProgressBar, ProgressStyle};
use jwalk::WalkDir;
use rayon::prelude::*;

// Extension preference when multiple files share the same stem (e.g. ARW + HIF pairs).
// Earlier in the list wins.
const PREFERRED_EXTS: &[&str] = &["arw", "jpg", "jpeg", "tif", "tiff", "hif", "heic"];

// JPEG EXIF lives in the APP1 segment near the start of the file; 128 KB is a safe ceiling.
const JPEG_READ_LIMIT: u64 = 128 * 1024;
// ARW/TIFF files store rational values (like FocalLength) at offsets that can exceed 1 MB
// into the file due to embedded previews placed before the metadata.
const RAW_READ_LIMIT: u64 = 4 * 1024 * 1024;

// One reusable buffer per rayon thread — eliminates a large allocation per file.
thread_local! {
    static HEADER_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(RAW_READ_LIMIT as usize));
}

fn exif_ascii(field: &exif::Field) -> Option<String> {
    if let exif::Value::Ascii(ref components) = field.value {
        // Ascii values are split on null bytes into components; rejoin non-empty ones.
        let s = components.iter()
            .flat_map(|c| c.iter().copied())
            .collect::<Vec<u8>>();
        let s = String::from_utf8_lossy(&s).trim().to_string();
        if !s.is_empty() { Some(s) } else { None }
    } else {
        None
    }
}

struct FileResult {
    focal_length: Option<String>,
    camera: Option<String>,
    lens: Option<String>,
    error: Option<String>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
    let dir_paths: Vec<&str> = args.iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .collect();

    if dir_paths.is_empty() {
        eprintln!("Usage: extractor-rust <path> [path...] [--verbose]");
        return;
    }

    // jwalk traverses the directory tree in parallel across rayon threads.
    // Deduplication by (parent_dir, stem): keep only the highest-priority extension per shot.
    // Keying by parent ensures DSC00001 in two different folders are treated as separate shots.
    let mut by_stem: HashMap<(PathBuf, String), PathBuf> = HashMap::new();
    for entry in dir_paths.iter()
        .flat_map(|dir| WalkDir::new(dir).into_iter())
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let parent = path.parent().unwrap_or(std::path::Path::new("")).to_path_buf();
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

        let Some(new_rank) = PREFERRED_EXTS.iter().position(|&e| e == ext) else {
            continue; // skip videos, sidecars, .DS_Store, etc.
        };

        let key = (parent, stem);
        let cur_rank = by_stem.get(&key).and_then(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .and_then(|e| PREFERRED_EXTS.iter().position(|&x| x == e))
        });

        match cur_rank {
            Some(c) if new_rank < c => { by_stem.insert(key, path); }
            None => { by_stem.insert(key, path); }
            _ => {}
        }
    }

    let mut paths: Vec<PathBuf> = by_stem.into_values().collect();
    paths.sort();

    let total = paths.len();
    let bar = ProgressBar::new(total as u64);
    bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})"
        )
        .unwrap()
        .progress_chars("=>-"),
    );

    // Read EXIF in parallel across all CPU cores, loading only the file header.
    // rayon preserves output order for indexed iterators, so results align with sorted paths.
    let results: Vec<FileResult> = paths
        .par_iter()
        .map(|path| {
            let file = match fs::File::open(path) {
                Ok(f) => f,
                Err(e) => return FileResult { focal_length: None, camera: None, lens: None, error: Some(e.to_string()) },
            };

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            let read_limit = match ext.as_str() {
                "jpg" | "jpeg" => JPEG_READ_LIMIT,
                _ => RAW_READ_LIMIT,
            };

            let result = HEADER_BUF.with(|cell| {
                let mut buf = cell.borrow_mut();
                buf.clear();

                if let Err(e) = file.take(read_limit).read_to_end(&mut *buf) {
                    return FileResult { focal_length: None, camera: None, lens: None, error: Some(e.to_string()) };
                }

                // read_from_container requires Seek; Cursor over the in-memory slice provides it.
                let exif = match exif::Reader::new()
                    .read_from_container(&mut BufReader::new(Cursor::new(buf.as_slice())))
                {
                    Ok(e) => e,
                    Err(e) => return FileResult { focal_length: None, camera: None, lens: None, error: Some(e.to_string()) },
                };

                let mut focal_length = None;
                let mut make = None;
                let mut model = None;
                let mut lens = None;

                for field in exif.fields() {
                    match field.tag {
                        exif::Tag::FocalLength => focal_length = Some(field.display_value().to_string()),
                        exif::Tag::Make => make = exif_ascii(field),
                        exif::Tag::Model => model = exif_ascii(field),
                        exif::Tag::LensModel => lens = exif_ascii(field),
                        _ => {}
                    }
                }

                let camera = match (make, model) {
                    (Some(mk), Some(mo)) => Some(format!("{} {}", mk, mo)),
                    (None, Some(mo)) => Some(mo),
                    (Some(mk), None) => Some(mk),
                    (None, None) => None,
                };

                FileResult { focal_length, camera, lens, error: None }
            });

            bar.inc(1);
            result
        })
        .collect();

    bar.finish_with_message("done");

    let mut focal_lengths: Vec<String> = Vec::new();
    let mut cameras: Vec<String> = Vec::new();
    let mut lenses: Vec<String> = Vec::new();
    let mut skipped: Vec<(&PathBuf, &str)> = Vec::new();

    for (path, result) in paths.iter().zip(results.iter()) {
        if let Some(fl) = &result.focal_length { focal_lengths.push(fl.clone()); }
        if let Some(cam) = &result.camera { cameras.push(cam.clone()); }
        if let Some(lens) = &result.lens { lenses.push(lens.clone()); }
        if let Some(err) = &result.error { skipped.push((path, err)); }
    }

    println!("\nTotal images scanned: {}", total);

    print_bar_chart("Focal Length Frequency Counter", &focal_lengths, |s| format!("{}mm", s));
    print_bar_chart("Camera Frequency Counter", &cameras, |s| s.to_string());
    print_bar_chart("Lens Frequency Counter", &lenses, |s| s.to_string());

    if !skipped.is_empty() {
        println!("\nSkipped {} file(s) with unreadable EXIF (run with --verbose to list them)", skipped.len());
        if verbose {
            for (path, reason) in &skipped {
                println!("  {} ({})", path.file_name().unwrap_or_default().to_string_lossy(), reason);
            }
        }
    }
}

fn print_bar_chart(title: &str, values: &[String], format_label: impl Fn(&str) -> String) {
    let mut counts: HashMap<&str, u32> = HashMap::new();
    for v in values {
        *counts.entry(v.as_str()).or_insert(0) += 1;
    }

    if counts.is_empty() {
        return;
    }

    let mut sorted: Vec<(&&str, &u32)> = counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    let max_count = *sorted[0].1;
    let bar_width = 40usize;
    let label_width = sorted.iter().map(|(k, _)| format_label(k).len()).max().unwrap_or(1);

    println!("\n{}:\n", title);
    for (label, count) in &sorted {
        let filled = ((**count as f64 / max_count as f64) * bar_width as f64).round() as usize;
        let formatted = format_label(label);
        println!("  {:<width$}  {}  {}", formatted, "█".repeat(filled), count, width = label_width);
    }
}
