use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

/// Lossless JSON representation for arbitrary bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "encoding", rename_all = "lowercase", deny_unknown_fields)]
pub enum EncodedBytes {
    /// Valid UTF-8 bytes.
    Utf8 {
        /// Exact UTF-8 text.
        text: String,
    },
    /// Bytes that are not valid UTF-8.
    Base64 {
        /// Canonical RFC 4648 base64.
        bytes: String,
    },
}

impl EncodedBytes {
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        match String::from_utf8(bytes.to_vec()) {
            Ok(text) => Self::Utf8 { text },
            Err(error) => Self::Base64 {
                bytes: STANDARD.encode(error.into_bytes()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EncodedBytes;

    #[test]
    fn encoded_bytes_uses_base64_when_path_bytes_are_invalid_utf8() {
        // Given: arbitrary path bytes that are not UTF-8.
        let raw_path = b"invalid-\xff.txt";

        // When: the path is encoded for authoritative JSON.
        let encoded = EncodedBytes::from_bytes(raw_path);

        // Then: no lossy replacement text is introduced.
        assert_eq!(
            encoded,
            EncodedBytes::Base64 {
                bytes: "aW52YWxpZC3/LnR4dA==".to_owned()
            }
        );
    }
}
