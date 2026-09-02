use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn verify_pack_signature(pack_path: &Path, expected_hex: &str) -> bool {
    sha256_file(pack_path).map(|h| h == expected_hex).unwrap_or(false)
}
