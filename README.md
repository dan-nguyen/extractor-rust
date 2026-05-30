# extractor-rust

A command-line tool that reads EXIF metadata from a directory of images and prints a focal length frequency count.

This is a Rust rewrite of the original [ExifReader Java project](https://github.com/dtnguyen/extractor), updated to support modern Sony camera file formats.

## What's new

- **Rewritten in Rust** — replaces the original Java + `metadata-extractor` library implementation
- **Sony ARW support** — reads EXIF directly from Sony RAW files
- **Broader format coverage** — handles JPEG, TIFF, ARW, and HEIF-based formats
- **Format deduplication** — when a shoot produces paired files (e.g. `DSC01234.ARW` + `DSC01234.HIF`), the highest-priority format is read once per shot, avoiding double-counting

## Usage

```
cargo run -- /path/to/images
```

## Supported formats

Priority order when multiple files share the same base name:

1. ARW (Sony RAW)
2. JPEG / JPG
3. TIFF / TIF
4. HIF / HEIC

## Dependencies

- [kamadak-exif](https://crates.io/crates/kamadak-exif) 0.6
