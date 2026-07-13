use std::{io, path::Path};

use crate::{
    bytes::{LineRange, canonical_block, parse_lines},
    evidence::{EvidenceBundle, decoded_bytes},
    identity::{BlockIdentityInput, block_id, source_sha256},
    model::EvidenceBlock,
};

use super::{
    BlockVerification, VerificationStatus, VerifyError, filesystem::path_from_bytes,
    policy::MAX_VERIFY_SOURCE_BYTES,
};

pub(super) struct LiveAttempt {
    pub(super) blocks: Vec<BlockVerification>,
    pub(super) raced: bool,
}

pub(super) struct SourceReader<'a, R> {
    read: &'a mut R,
    remaining: usize,
}

impl<'a, R> SourceReader<'a, R>
where
    R: FnMut(&Path, usize) -> io::Result<Vec<u8>>,
{
    pub(super) const fn new(read: &'a mut R) -> Self {
        Self {
            read,
            remaining: MAX_VERIFY_SOURCE_BYTES,
        }
    }

    fn read(&mut self, path: &Path, maximum: usize) -> Result<Option<Vec<u8>>, VerifyError> {
        let raw = match (self.read)(path, maximum) {
            Ok(raw) => raw,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(VerifyError::SourceRead(error)),
        };
        let Some(next) = self.remaining.checked_sub(raw.len()) else {
            return Err(VerifyError::ResourceLimit);
        };
        self.remaining = next;
        Ok(Some(raw))
    }
}

struct LiveSource<'a> {
    raw: &'a [u8],
    raw_path: &'a [u8],
    digest: &'a str,
}

pub(super) fn attempt<R>(
    bundle: &EvidenceBundle,
    reader: &mut SourceReader<'_, R>,
) -> Result<LiveAttempt, VerifyError>
where
    R: FnMut(&Path, usize) -> io::Result<Vec<u8>>,
{
    let mut blocks = Vec::with_capacity(bundle.blocks.len());
    let mut snapshots = Vec::new();
    for block in &bundle.blocks {
        let raw_path = decoded_bytes(&block.path).map_err(|_| VerifyError::UnsupportedSchema)?;
        let Some(path) = path_from_bytes(&raw_path) else {
            return Err(VerifyError::UnsafePath);
        };
        match reader.read(&path, bundle.budget.max_file_bytes.get())? {
            Some(raw) => {
                let digest = source_sha256(&raw);
                let status = live_status(
                    block,
                    &LiveSource {
                        raw: &raw,
                        raw_path: &raw_path,
                        digest: &digest,
                    },
                )?;
                snapshots.push((path, digest));
                blocks.push(block_report(block, status));
            }
            None => blocks.push(block_report(block, VerificationStatus::Stale)),
        }
    }
    let mut raced = false;
    for (path, digest) in snapshots {
        let current = reader.read(&path, bundle.budget.max_file_bytes.get())?;
        if current.is_none_or(|raw| source_sha256(&raw) != digest) {
            raced = true;
        }
    }
    Ok(LiveAttempt { blocks, raced })
}

fn live_status(
    block: &EvidenceBlock,
    source: &LiveSource<'_>,
) -> Result<VerificationStatus, VerifyError> {
    if source.digest != block.source_sha256 {
        return Ok(VerificationStatus::SourceDrift);
    }
    let Ok(lines) = parse_lines(source.raw) else {
        return Ok(VerificationStatus::Forged);
    };
    let Some(start) = block.line_start.get().checked_sub(1) else {
        return Ok(VerificationStatus::Forged);
    };
    let Some(end) = block.line_end.get().checked_sub(1) else {
        return Ok(VerificationStatus::Forged);
    };
    let Ok(canonical) = canonical_block(&lines, LineRange { start, end }) else {
        return Ok(VerificationStatus::Forged);
    };
    let expected_content =
        decoded_bytes(&block.content).map_err(|_| VerifyError::UnsupportedSchema)?;
    let expected_id = block_id(BlockIdentityInput {
        path: source.raw_path,
        line_start: block.line_start,
        line_end: block.line_end,
        content: &canonical.content,
    })?;
    if canonical.content.as_slice() != expected_content.as_ref()
        || canonical.byte_start != block.byte_start.get()
        || canonical.byte_end != block.byte_end.get()
        || expected_id != block.block_id
    {
        return Ok(VerificationStatus::Forged);
    }
    Ok(VerificationStatus::Verified)
}

fn block_report(block: &EvidenceBlock, status: VerificationStatus) -> BlockVerification {
    BlockVerification {
        block_id: block.block_id.clone(),
        status,
    }
}
