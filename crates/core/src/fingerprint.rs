use blake3::Hasher;

pub fn hash_content(content: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(content);
    hasher.finalize().to_hex().to_string()
}

pub fn hash_file(path: &std::path::Path) -> std::io::Result<String> {
    let content = std::fs::read(path)?;
    Ok(hash_content(&content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consistent_hash() {
        let content = b"Hello, world!";
        let hash1 = hash_content(content);
        let hash2 = hash_content(content);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn different_content_different_hash() {
        let hash1 = hash_content(b"Hello");
        let hash2 = hash_content(b"World");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn hash_is_64_chars() {
        let hash = hash_content(b"test");
        assert_eq!(hash.len(), 64);
    }
}
