use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::model::LineNumber;

const BLOCK_DOMAIN: &[u8] = b"imnotrag:block:v1\0";

/// Inputs to the frozen block identity preimage.
#[derive(Debug, Clone, Copy)]
pub struct BlockIdentityInput<'a> {
    /// Raw root-relative path bytes.
    pub path: &'a [u8],
    /// First one-based line.
    pub line_start: LineNumber,
    /// Last one-based line.
    pub line_end: LineNumber,
    /// LF-canonical content bytes.
    pub content: &'a [u8],
}

/// Block identity construction errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IdentityError {
    /// A frame cannot be represented by the required u64 length.
    #[error("block identity frame exceeds u64 length")]
    FrameTooLarge,
}

/// Computes the domain-separated, length-framed block identifier.
pub fn block_id(input: BlockIdentityInput<'_>) -> Result<String, IdentityError> {
    let mut digest = Sha256::new();
    digest.update(BLOCK_DOMAIN);
    update_frame(&mut digest, input.path)?;
    update_frame(&mut digest, input.line_start.get().to_string().as_bytes())?;
    update_frame(&mut digest, input.line_end.get().to_string().as_bytes())?;
    update_frame(&mut digest, input.content)?;
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn update_frame(digest: &mut Sha256, field: &[u8]) -> Result<(), IdentityError> {
    let length = u64::try_from(field.len()).map_err(|_| IdentityError::FrameTooLarge)?;
    digest.update(length.to_be_bytes());
    digest.update(field);
    Ok(())
}

/// Computes SHA-256 over the exact complete raw file bytes.
pub fn source_sha256(raw: &[u8]) -> String {
    hex::encode(Sha256::digest(raw))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{BlockIdentityInput, block_id, source_sha256};
    use crate::model::LineNumber;

    #[test]
    fn identity_matches_frozen_vector_when_config_fixture_is_hashed() {
        // Given: the exact frozen config path, line range, and bytes.
        let path = b"fixtures/multilingual/config/feature-flags.txt";
        let raw = b"FEATURE_FLAG enables safe rollout.\n\
            \xea\xb8\xb0\xeb\x8a\xa5 \xed\x94\x8c\xeb\x9e\x98\xea\xb7\xb8\xeb\x8a\x94 \
            \xeb\x8b\xa8\xea\xb3\x84\xec\xa0\x81 \xeb\xb0\xb0\xed\x8f\xac\xeb\xa5\xbc \
            \xec\xa7\x80\xec\x9b\x90\xed\x95\xa9\xeb\x8b\x88\xeb\x8b\xa4.\n\
            DEPLOY_REGION=ap-northeast-2\n";
        let content = raw.strip_suffix(b"\n").expect("fixture has final LF");
        let start = LineNumber::from_zero_based(0).expect("line one exists");
        let end = LineNumber::from_zero_based(2).expect("line three exists");

        // When: both identity digests are computed.
        let block = block_id(BlockIdentityInput {
            path,
            line_start: start,
            line_end: end,
            content,
        })
        .expect("fixture frame lengths fit u64");
        let source = source_sha256(raw);

        // Then: both exact contract vectors match.
        assert_eq!(
            block,
            "sha256:f84129bf3ddb191fd4315317a34fcf684121e3915f41c98ce2e59d697fd4e0bd"
        );
        assert_eq!(
            source,
            "84dda1450952dcb7c8221d843610a4b443b37c8a668eaade0b5c20fadd19bb65"
        );
    }

    proptest! {
        #[test]
        fn block_identity_changes_when_content_changes(content in prop::collection::vec(any::<u8>(), 0..128)) {
            // Given: one block identity input and a byte-appended variant.
            let start = LineNumber::from_zero_based(0).expect("line one exists");
            let mut changed = content.clone();
            changed.push(0x5a);

            // When: both block identifiers are computed.
            let original = block_id(BlockIdentityInput { path: b"a", line_start: start, line_end: start, content: &content })
                .expect("generated frame lengths fit u64");
            let modified = block_id(BlockIdentityInput { path: b"a", line_start: start, line_end: start, content: &changed })
                .expect("generated frame lengths fit u64");

            // Then: the length-framed digest changes.
            prop_assert_ne!(original, modified);
        }
    }
}
