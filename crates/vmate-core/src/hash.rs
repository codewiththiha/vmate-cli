//! SHA-256 helpers.

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Hash the contents of a file, returning lowercase hex.
pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(sha256_bytes(&bytes))
}

/// Hash a byte slice, returning lowercase hex.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Hash a string, returning lowercase hex.
pub fn sha256_str(s: &str) -> String {
    sha256_bytes(s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_deterministically() {
        assert_eq!(sha256_str("hello"), sha256_str("hello"));
        assert_ne!(sha256_str("hello"), sha256_str("world"));
    }
}
