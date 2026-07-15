pub struct Citation {
    pub path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub source_sha256: String,
}

pub fn literal_search(query: &[u8], source: &[u8]) -> bool {
    source.windows(query.len()).any(|window| window == query)
}
