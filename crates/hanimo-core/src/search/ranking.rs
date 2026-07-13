use std::cmp::Ordering;

use crate::model::EvidenceBlock;

#[derive(Debug)]
pub(super) struct RankedBlock {
    pub(super) block: EvidenceBlock,
    pub(super) raw_path: Vec<u8>,
}

pub(super) fn compare_ranked(left: &RankedBlock, right: &RankedBlock) -> Ordering {
    right
        .block
        .score
        .cmp(&left.block.score)
        .then_with(|| left.raw_path.cmp(&right.raw_path))
        .then_with(|| left.block.line_start.cmp(&right.block.line_start))
        .then_with(|| left.block.byte_start.cmp(&right.block.byte_start))
        .then_with(|| left.block.block_id.cmp(&right.block.block_id))
}
