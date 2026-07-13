use crate::{
    bytes::{SourceLine, contains, is_heading},
    model::{QueryPlan, ScoreComponents},
};

pub(crate) fn matched_terms(plan: &QueryPlan, content: &[u8]) -> Vec<String> {
    plan.quoted_phrases
        .iter()
        .chain(&plan.identifiers)
        .chain(&plan.terms)
        .filter(|term| contains(content, term.as_bytes()))
        .cloned()
        .collect()
}

pub(crate) fn score(plan: &QueryPlan, lines: &[SourceLine<'_>], content: &[u8]) -> ScoreComponents {
    let exact_phrase = component_any(&plan.quoted_phrases, content, 300);
    let identifier = component_any(&plan.identifiers, content, 250);
    let all_terms = component_all(&plan.terms, content, 150);
    let heading = u16::from(lines.iter().any(|line| is_heading(line.content))).saturating_mul(75);
    ScoreComponents {
        exact_phrase,
        identifier,
        all_terms,
        heading,
        path: 50,
        proximity: proximity(plan, lines),
    }
}

fn component_any(needles: &[String], content: &[u8], value: u16) -> u16 {
    u16::from(
        needles
            .iter()
            .any(|needle| contains(content, needle.as_bytes())),
    )
    .saturating_mul(value)
}

fn component_all(needles: &[String], content: &[u8], value: u16) -> u16 {
    u16::from(
        !needles.is_empty()
            && needles
                .iter()
                .all(|needle| contains(content, needle.as_bytes())),
    )
    .saturating_mul(value)
}

fn proximity(plan: &QueryPlan, lines: &[SourceLine<'_>]) -> u16 {
    let required = plan.quoted_phrases.iter().chain(&plan.identifiers);
    let positions: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            required
                .clone()
                .any(|needle| contains(line.content, needle.as_bytes()))
                .then_some(index)
        })
        .collect();
    let Some(first) = positions.first() else {
        return 0;
    };
    let Some(last) = positions.last() else {
        return 0;
    };
    let span = last.saturating_sub(*first).min(8);
    let penalty = u16::try_from(span).unwrap_or(8).saturating_mul(5);
    40_u16.saturating_sub(penalty)
}

pub(crate) fn reasons(components: ScoreComponents) -> Vec<String> {
    let entries = [
        (components.exact_phrase, "exact phrase"),
        (components.identifier, "identifier"),
        (components.all_terms, "all terms"),
        (components.heading, "heading"),
        (components.path, "path match"),
        (components.proximity, "proximity"),
    ];
    entries
        .into_iter()
        .filter(|(value, _)| *value > 0)
        .map(|(_, reason)| reason.to_owned())
        .collect()
}
