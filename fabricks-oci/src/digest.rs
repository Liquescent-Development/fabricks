//! Content-addressable digest computation.
//!
//! OCI uses SHA256 digests in the format `sha256:<hex>` to identify blobs.

use sha2::{Digest as Sha2Digest, Sha256};

/// Compute the SHA256 digest of content in OCI format.
///
/// Returns a string in the format `sha256:<64-char-hex>`.
///
/// # Example
///
/// ```
/// use fabricks_oci::digest::compute_digest;
///
/// let data = b"hello world";
/// let digest = compute_digest(data);
/// assert!(digest.starts_with("sha256:"));
/// assert_eq!(digest.len(), 7 + 64); // "sha256:" + 64 hex chars
/// ```
#[must_use]
pub fn compute_digest(content: &[u8]) -> String {
    let hash = Sha256::digest(content);
    format!("sha256:{}", hex::encode(hash))
}

/// Verify that content matches an expected digest.
///
/// # Errors
///
/// Returns an error if the computed digest doesn't match the expected digest.
pub fn verify_digest(content: &[u8], expected: &str) -> Result<(), DigestError> {
    let actual = compute_digest(content);
    if actual == expected {
        Ok(())
    } else {
        Err(DigestError::Mismatch {
            expected: expected.to_string(),
            actual,
        })
    }
}

/// Parse a digest string into algorithm and hash parts.
///
/// # Errors
///
/// Returns an error if the digest format is invalid.
pub fn parse_digest(digest: &str) -> Result<(&str, &str), DigestError> {
    let parts: Vec<&str> = digest.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(DigestError::InvalidFormat {
            digest: digest.to_string(),
        });
    }

    let algorithm = parts[0];
    let hash = parts[1];

    // Validate algorithm
    if algorithm != "sha256" && algorithm != "sha512" {
        return Err(DigestError::UnsupportedAlgorithm {
            algorithm: algorithm.to_string(),
        });
    }

    // Validate hash length
    let expected_len = match algorithm {
        "sha256" => 64,
        "sha512" => 128,
        _ => unreachable!(),
    };

    if hash.len() != expected_len {
        return Err(DigestError::InvalidFormat {
            digest: digest.to_string(),
        });
    }

    // Validate hex characters
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DigestError::InvalidFormat {
            digest: digest.to_string(),
        });
    }

    Ok((algorithm, hash))
}

/// Errors related to digest operations.
#[derive(Debug, thiserror::Error)]
pub enum DigestError {
    /// Digest format is invalid.
    #[error("invalid digest format: {digest}")]
    InvalidFormat {
        /// The invalid digest string.
        digest: String,
    },

    /// Digest algorithm is not supported.
    #[error("unsupported digest algorithm: {algorithm}")]
    UnsupportedAlgorithm {
        /// The unsupported algorithm.
        algorithm: String,
    },

    /// Computed digest doesn't match expected.
    #[error("digest mismatch: expected {expected}, got {actual}")]
    Mismatch {
        /// Expected digest.
        expected: String,
        /// Actual computed digest.
        actual: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_digest() {
        let data = b"hello world";
        let digest = compute_digest(data);

        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), 7 + 64);
        // Known SHA256 of "hello world"
        assert_eq!(
            digest,
            "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_verify_digest_success() {
        let data = b"hello world";
        let expected = "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_digest(data, expected).is_ok());
    }

    #[test]
    fn test_verify_digest_failure() {
        let data = b"hello world";
        let wrong = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        assert!(verify_digest(data, wrong).is_err());
    }

    #[test]
    fn test_parse_digest_valid() {
        let digest = "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let result = parse_digest(digest);
        assert!(result.is_ok());

        let (algo, hash) = result.unwrap_or_else(|e| panic!("unexpected error: {e}"));
        assert_eq!(algo, "sha256");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_parse_digest_invalid_format() {
        assert!(parse_digest("invalid").is_err());
        assert!(parse_digest("sha256:tooshort").is_err());
        assert!(parse_digest("md5:d41d8cd98f00b204e9800998ecf8427e").is_err());
    }
}
