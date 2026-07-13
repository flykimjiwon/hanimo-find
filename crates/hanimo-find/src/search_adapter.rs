use std::path::Path;

use hanimo_core::{
    EvidenceBundle, EvidenceError, QueryPlan, SearchError, assemble_bundle,
    model::{Budget, QueryLimitError, validate_query_bytes, validate_query_literal_count},
    search,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum SearchAdapterError {
    #[error("query must contain at least one non-empty literal")]
    EmptyQuery,
    #[error("query contains an unterminated quoted phrase")]
    UnterminatedQuote,
    #[error(transparent)]
    QueryLimit(#[from] QueryLimitError),
    #[error("search root is not valid UTF-8")]
    InvalidRoot,
    #[error(transparent)]
    Search(#[from] SearchError),
    #[error(transparent)]
    Evidence(#[from] EvidenceError),
}

struct ParsedQuery {
    quoted_phrases: Vec<String>,
    identifiers: Vec<String>,
    terms: Vec<String>,
}

impl SearchAdapterError {
    pub(crate) const fn is_usage(&self) -> bool {
        matches!(
            self,
            Self::EmptyQuery | Self::UnterminatedQuote | Self::QueryLimit(_)
        )
    }
}

pub(crate) fn search_evidence(
    query: &str,
    root: &Path,
) -> Result<EvidenceBundle, SearchAdapterError> {
    let root_text = root.to_str().ok_or(SearchAdapterError::InvalidRoot)?;
    let parsed = parse_query(query)?;
    let plan = QueryPlan {
        schema_version: hanimo_core::model::SCHEMA_VERSION.to_owned(),
        query: query.to_owned(),
        root: root_text.to_owned(),
        quoted_phrases: parsed.quoted_phrases,
        identifiers: parsed.identifiers,
        terms: parsed.terms,
        budget: Budget::default(),
    };
    let result = search(root, &plan)?;
    Ok(assemble_bundle(&plan, result)?)
}

fn parse_query(query: &str) -> Result<ParsedQuery, SearchAdapterError> {
    validate_query_bytes(query.len())?;
    let mut quoted = Vec::new();
    let mut outside = String::new();
    let mut phrase = String::new();
    let mut in_quote = false;
    let mut literal_count = 0_usize;
    for character in query.chars() {
        if character == '"' {
            if in_quote {
                if phrase.is_empty() {
                    return Err(SearchAdapterError::EmptyQuery);
                }
                literal_count = literal_count.saturating_add(1);
                validate_query_literal_count(literal_count)?;
                quoted.push(std::mem::take(&mut phrase));
            } else {
                outside.push(' ');
            }
            in_quote = !in_quote;
        } else if in_quote {
            phrase.push(character);
        } else {
            outside.push(character);
        }
    }
    if in_quote {
        return Err(SearchAdapterError::UnterminatedQuote);
    }
    let mut identifiers = Vec::new();
    let mut terms = Vec::new();
    for token in outside.split_whitespace() {
        let literal = token.trim_matches(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        });
        if literal.is_empty() {
            continue;
        }
        literal_count = literal_count.saturating_add(1);
        validate_query_literal_count(literal_count)?;
        if is_identifier(literal) {
            identifiers.push(literal.to_owned());
        } else {
            terms.push(literal.to_owned());
        }
    }
    if quoted.is_empty() && identifiers.is_empty() && terms.is_empty() {
        return Err(SearchAdapterError::EmptyQuery);
    }
    Ok(ParsedQuery {
        quoted_phrases: quoted,
        identifiers,
        terms,
    })
}

fn is_identifier(literal: &str) -> bool {
    literal
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        && literal.bytes().any(|byte| byte.is_ascii_uppercase())
}
