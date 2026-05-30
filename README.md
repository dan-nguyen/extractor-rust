# extractor-rust

A command-line tool that reads EXIF metadata from a directory of images and prints a focal length frequency chart.

This is a Rust rewrite of the original [ExifReader Java project](https://github.com/dan-nguyen/extractor), updated to support a wider range of camera formats.

## What's new

- **Rewritten in Rust** — replaces the original Java + `metadata-extractor` library implementation
- **RAW file support** — reads EXIF directly from camera RAW files (e.g. ARW)
- **Broader format coverage** — handles JPEG, TIFF, RAW, and HEIF-based formats
- **Format deduplication** — when a shoot produces paired files (e.g. `IMG_1234.RAW` + `IMG_1234.JPG`), the highest-priority format is read once per shot, avoiding double-counting
- **Parallel scanning** — walks directories and reads EXIF concurrently across all CPU cores
- **Progress bar** — shows live progress, elapsed time, and ETA during scanning
- **Bar chart output** — focal lengths sorted by frequency with a scaled ASCII bar chart

## Setup

1. Install Rust via [rustup](https://rustup.rs):
   ```
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. Clone the repo and build:
   ```
   git clone https://github.com/dan-nguyen/extractor-rust
   cd extractor-rust
   cargo build --release
   ```

## Usage

```
./target/release/extractor-rust /path/to/images
```

To list files that could not be read:

```
./target/release/extractor-rust /path/to/images --verbose
```

## Output

```
Scanning 494 images...
⠋ [00:00:02] [========================================] 494/494 (eta: 0s)

Total images scanned: 494

Focal Length Frequency Counter:

  24mm  ████████████████████████████████████████  139
  70mm  ████████████████████████████              97
  68mm  ██████                                    13
  ...

Skipped 12 file(s) with unreadable EXIF (run with --verbose to list them)
```

## Supported formats

Priority order when multiple files share the same base name:

1. ARW (RAW)
2. JPEG / JPG
3. TIFF / TIF
4. HIF / HEIC

## Dependencies

- [indicatif](https://crates.io/crates/indicatif) 0.17
- [jwalk](https://crates.io/crates/jwalk) 0.8
- [kamadak-exif](https://crates.io/crates/kamadak-exif) 0.6
- [rayon](https://crates.io/crates/rayon) 1
