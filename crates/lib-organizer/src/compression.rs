use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

const ZSTD_EXTENSION: &str = "zst";

pub fn compress_file(src: &Path, dst: &Path, level: i32) -> anyhow::Result<u64> {
    let input = File::open(src)?;
    let reader = BufReader::new(input);

    let output = File::create(dst)?;
    let writer = BufWriter::new(output);

    let mut encoder = zstd::Encoder::new(writer, level)?;

    let mut buf_reader = reader;
    std::io::copy(&mut buf_reader, &mut encoder)?;

    encoder.finish()?;

    let metadata = std::fs::metadata(dst)?;
    Ok(metadata.len())
}

pub fn decompress_file(src: &Path, dst: &Path) -> anyhow::Result<u64> {
    let input = File::open(src)?;
    let reader = BufReader::new(input);

    let output = File::create(dst)?;
    let mut writer = BufWriter::new(output);

    let mut decoder = zstd::Decoder::new(reader)?;
    std::io::copy(&mut decoder, &mut writer)?;

    writer.flush()?;

    let metadata = std::fs::metadata(dst)?;
    Ok(metadata.len())
}

pub fn compress_bytes(data: &[u8], level: i32) -> anyhow::Result<Vec<u8>> {
    Ok(zstd::encode_all(data, level)?)
}

pub fn decompress_bytes(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    Ok(zstd::decode_all(data)?)
}

pub fn compressed_path(original: &Path) -> std::path::PathBuf {
    let mut new_path = original.as_os_str().to_owned();
    new_path.push(".");
    new_path.push(ZSTD_EXTENSION);
    std::path::PathBuf::from(new_path)
}

pub fn is_compressed(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e == ZSTD_EXTENSION)
        .unwrap_or(false)
}

pub fn original_path(compressed: &Path) -> Option<std::path::PathBuf> {
    if !is_compressed(compressed) {
        return None;
    }

    let s = compressed.to_str()?;
    let trimmed = s.strip_suffix(&format!(".{}", ZSTD_EXTENSION))?;
    Some(std::path::PathBuf::from(trimmed))
}

pub fn compression_ratio(original_size: u64, compressed_size: u64) -> f64 {
    if original_size == 0 {
        return 0.0;
    }
    1.0 - (compressed_size as f64 / original_size as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn compress_decompress_roundtrip() {
        let dir = TempDir::new().unwrap();

        let original = dir.path().join("test.pdf");
        let compressed = dir.path().join("test.pdf.zst");
        let decompressed = dir.path().join("test_restored.pdf");

        let content = b"This is test content for compression.";
        std::fs::write(&original, content).unwrap();

        compress_file(&original, &compressed, 3).unwrap();
        assert!(compressed.exists());

        decompress_file(&compressed, &decompressed).unwrap();
        let restored = std::fs::read(&decompressed).unwrap();

        assert_eq!(content.as_slice(), restored.as_slice());
    }

    #[test]
    fn compress_bytes_roundtrip() {
        let original = b"Test data for byte compression";
        let compressed = compress_bytes(original, 3).unwrap();
        let restored = decompress_bytes(&compressed).unwrap();

        assert_eq!(original.as_slice(), restored.as_slice());
    }

    #[test]
    fn compressed_path_generation() {
        let path = Path::new("/lib/programming/rust/book.pdf");
        let comp = compressed_path(path);
        assert_eq!(comp.to_str().unwrap(), "/lib/programming/rust/book.pdf.zst");
    }

    #[test]
    fn is_compressed_detection() {
        assert!(is_compressed(Path::new("book.pdf.zst")));
        assert!(!is_compressed(Path::new("book.pdf")));
    }

    #[test]
    fn original_path_recovery() {
        let compressed = Path::new("/lib/book.pdf.zst");
        let original = original_path(compressed).unwrap();
        assert_eq!(original.to_str().unwrap(), "/lib/book.pdf");
    }

    #[test]
    fn compression_ratio_calculation() {
        let ratio = compression_ratio(1000, 600);
        assert!((ratio - 0.4).abs() < 0.001);
    }
}
