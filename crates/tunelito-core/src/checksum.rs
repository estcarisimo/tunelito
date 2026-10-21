use sha2::{Digest, Sha256};

use crate::error::Error;

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> Result<(), Error> {
    let expected = expected_hex.trim();
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::checksum_mismatch());
    }
    let got = sha256_hex(bytes);
    if !got.eq_ignore_ascii_case(expected) {
        return Err(Error::checksum_mismatch());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_abc() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn mismatch_is_rejected() {
        assert!(verify_sha256(b"abc", "00").is_err());
        assert!(verify_sha256(b"abc", &"ab".repeat(32)).is_err());
        assert!(verify_sha256(
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        )
        .is_ok());
    }
}
