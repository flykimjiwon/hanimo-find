use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum BytesError {
    #[error("source offset overflow")]
    OffsetOverflow,
    #[error("invalid canonical line range")]
    InvalidRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceLine<'a> {
    pub(crate) content: &'a [u8],
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LineRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalBlock {
    pub(crate) content: Vec<u8>,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
}

pub(crate) fn parse_lines(raw: &[u8]) -> Result<Vec<SourceLine<'_>>, BytesError> {
    let mut lines = Vec::new();
    let mut cursor = 0_usize;
    for chunk in raw.split_inclusive(|byte| *byte == b'\n') {
        let content_length = terminated_content_length(chunk);
        let byte_end = cursor
            .checked_add(content_length)
            .ok_or(BytesError::OffsetOverflow)?;
        let content = chunk
            .get(..content_length)
            .ok_or(BytesError::InvalidRange)?;
        lines.push(SourceLine {
            content,
            byte_start: cursor,
            byte_end,
        });
        cursor = cursor
            .checked_add(chunk.len())
            .ok_or(BytesError::OffsetOverflow)?;
    }
    Ok(lines)
}

fn terminated_content_length(chunk: &[u8]) -> usize {
    chunk.strip_suffix(b"\n").map_or(chunk.len(), |without_lf| {
        without_lf.strip_suffix(b"\r").unwrap_or(without_lf).len()
    })
}

pub(crate) fn canonical_block(
    lines: &[SourceLine<'_>],
    range: LineRange,
) -> Result<CanonicalBlock, BytesError> {
    let selected = lines
        .get(range.start..=range.end)
        .ok_or(BytesError::InvalidRange)?;
    let first = selected.first().ok_or(BytesError::InvalidRange)?;
    let last = selected.last().ok_or(BytesError::InvalidRange)?;
    let separator_count = selected.len().saturating_sub(1);
    let content_length = selected.iter().try_fold(separator_count, |length, line| {
        length.checked_add(line.content.len())
    });
    let mut content = Vec::with_capacity(content_length.ok_or(BytesError::OffsetOverflow)?);
    for (index, line) in selected.iter().enumerate() {
        if index > 0 {
            content.push(b'\n');
        }
        content.extend_from_slice(line.content);
    }
    Ok(CanonicalBlock {
        content,
        byte_start: first.byte_start,
        byte_end: last.byte_end,
    })
}

pub(crate) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    occurrence_count(haystack, needle) > 0
}

pub(crate) fn occurrence_count(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

pub(crate) fn is_heading(line: &[u8]) -> bool {
    let level = line.iter().take_while(|byte| **byte == b'#').count();
    (1..=6).contains(&level) && line.get(level).is_none_or(|byte| *byte == b' ')
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{LineRange, canonical_block, parse_lines};

    #[test]
    fn parse_lines_preserves_bare_cr_when_mixed_endings_are_present() {
        // Given: LF, CRLF, bare CR, and a final terminator.
        let raw = b"one\r\ntwo\nthree\rfour\n";

        // When: the raw bytes are parsed and canonicalized.
        let lines = parse_lines(raw).expect("fixture offsets fit usize");
        let block =
            canonical_block(&lines, LineRange { start: 0, end: 2 }).expect("fixture range exists");

        // Then: terminators normalize to LF while the bare CR remains content.
        assert_eq!(block.content, b"one\ntwo\nthree\rfour");
        assert_eq!(block.byte_end, raw.len() - 1);
    }

    proptest! {
        #[test]
        fn canonical_bytes_match_when_lf_and_crlf_encode_same_lines(
            logical_lines in prop::collection::vec("[A-Za-z0-9]{0,16}", 1..8),
        ) {
            // Given: equivalent randomly generated logical lines.
            let lf = format!("{}\n", logical_lines.join("\n"));
            let crlf = format!("{}\r\n", logical_lines.join("\r\n"));

            // When: both physical encodings are canonicalized.
            let lf_lines = parse_lines(lf.as_bytes()).expect("generated offsets fit usize");
            let crlf_lines = parse_lines(crlf.as_bytes()).expect("generated offsets fit usize");
            let end = logical_lines.len().saturating_sub(1);
            let lf_block = canonical_block(&lf_lines, LineRange { start: 0, end })
                .expect("generated range exists");
            let crlf_block = canonical_block(&crlf_lines, LineRange { start: 0, end })
                .expect("generated range exists");

            // Then: the canonical byte strings are identical.
            prop_assert_eq!(lf_block.content, crlf_block.content);
        }
    }
}
