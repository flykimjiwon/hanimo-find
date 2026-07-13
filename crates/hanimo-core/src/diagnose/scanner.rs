use std::path::Path;

use sha2::{Digest, Sha256};

use super::{
    Citation, DIRECT_RULES, DiagnoseBudget, DiagnoseError, Finding, RagDiagnosis, RuleId,
    SCHEMA_VERSION, Severity, filesystem,
};

pub(super) fn diagnose(root: &Path, budget: DiagnoseBudget) -> Result<RagDiagnosis, DiagnoseError> {
    let mut scan = ScanState::new();
    filesystem::scan_sources(root, budget, |path, bytes| scan.visit(path, bytes))?;
    Ok(scan.finish())
}

struct ScanState {
    hasher: Sha256,
    direct: Vec<Option<Citation>>,
    missing_line: Option<Citation>,
    retrieval: Option<Citation>,
    has_freshness: bool,
    has_exact: bool,
}

struct SourceLine<'a> {
    path: &'a str,
    number: usize,
    text: &'a str,
}

impl ScanState {
    fn new() -> Self {
        Self {
            hasher: Sha256::new(),
            direct: std::iter::repeat_n(None, DIRECT_RULES.len()).collect(),
            missing_line: None,
            retrieval: None,
            has_freshness: false,
            has_exact: false,
        }
    }

    fn visit(&mut self, path: &str, bytes: &[u8]) {
        self.hasher.update(path.as_bytes());
        self.hasher.update([0]);
        self.hasher.update(Sha256::digest(bytes));
        let Ok(text) = std::str::from_utf8(bytes) else {
            return;
        };
        for (index, text) in text.lines().enumerate() {
            self.visit_line(&SourceLine {
                path,
                number: index.saturating_add(1),
                text,
            });
        }
    }

    fn visit_line(&mut self, line: &SourceLine<'_>) {
        let trimmed = line.text.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '*']) || trimmed.starts_with("//") {
            return;
        }
        let text = trimmed.to_ascii_lowercase();
        for (citation, &(_, needles)) in self.direct.iter_mut().zip(DIRECT_RULES) {
            if citation.is_none() && contains_any(&text, needles) {
                *citation = Some(citation_for(line.path, line.number));
            }
        }
        if self.missing_line.is_none()
            && text.contains("citation")
            && text.contains("source")
            && !text.contains("line")
        {
            self.missing_line = Some(citation_for(line.path, line.number));
        }
        if self.retrieval.is_none()
            && contains_any(&text, "similarity_search(|as_retriever(|vectorstore")
        {
            self.retrieval = Some(citation_for(line.path, line.number));
        }
        self.has_freshness |= contains_any(&text, "source_sha256|freshness|rehash|verify_source");
        self.has_exact |= contains_any(&text, "literal_search|exact_search|regex_search|bm25");
    }

    fn finish(self) -> RagDiagnosis {
        let mut findings = Vec::new();
        for (citation, &(rule_id, _)) in self.direct.into_iter().zip(DIRECT_RULES) {
            if let Some(citation) = citation {
                findings.push(finding(rule_id, citation));
            }
        }
        if let Some(citation) = self.missing_line {
            findings.push(finding(RuleId::MissingLineCitations, citation));
        }
        if let Some(citation) = self.retrieval {
            if !self.has_freshness {
                findings.push(finding(
                    RuleId::MissingFreshnessValidation,
                    citation.clone(),
                ));
            }
            if !self.has_exact {
                findings.push(finding(RuleId::MissingExactSearchFallback, citation));
            }
        }
        let count = findings.len();
        RagDiagnosis {
            schema_version: SCHEMA_VERSION,
            bundle_sha256: hex::encode(self.hasher.finalize()),
            findings,
            summary: format!("Detected {count} source-cited RAG risk patterns."),
        }
    }
}

fn citation_for(path: &str, line: usize) -> Citation {
    Citation {
        path: path.to_owned(),
        line,
    }
}

fn finding(rule_id: RuleId, citation: Citation) -> Finding {
    Finding {
        rule_id,
        severity: Severity::Warning,
        message: format!("{}: inspectable RAG risk detected.", rule_id.as_str()),
        citations: vec![citation],
    }
}

fn contains_any(text: &str, needles: &str) -> bool {
    needles.split('|').any(|needle| text.contains(needle))
}
