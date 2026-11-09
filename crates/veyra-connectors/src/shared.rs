//! Building blocks shared by connectors.

use std::collections::HashMap;
use std::path::{Component, Path};

use jiff::Timestamp;
use veyra_domain::source::RawUsage;
use veyra_domain::time::UtcMillis;

/// Plain path components, or `None` if the path has any other kind of component. Source
/// file allow-lists match on these, so `..`, roots and prefixes never match.
pub fn plain_components(path: &Path) -> Option<Vec<&str>> {
    path.components()
        .map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect()
}

/// An RFC 3339 timestamp (e.g. `2026-01-02T03:04:05.000Z`) as UTC milliseconds.
pub fn utc_millis(text: &str) -> Option<UtcMillis> {
    text.parse::<Timestamp>()
        .ok()
        .map(|t| UtcMillis(t.as_millisecond()))
}

/// Keeps one record per natural key: the observation with the highest precedence (the
/// earlier one on a tie), with the tool calls of every observation combined.
#[derive(Debug, Default)]
pub struct BestByKey {
    records: Vec<RawUsage>,
    index: HashMap<String, usize>,
}

impl BestByKey {
    pub fn add(&mut self, candidate: RawUsage) {
        let Some(&at) = self.index.get(&candidate.natural_key) else {
            self.index
                .insert(candidate.natural_key.clone(), self.records.len());
            self.records.push(candidate);
            return;
        };
        let current = &mut self.records[at];
        let mut calls = std::mem::take(&mut current.tool_calls);
        for call in &candidate.tool_calls {
            if !calls.iter().any(|c| c.id == call.id) {
                calls.push(call.clone());
            }
        }
        let better = candidate.precedence > current.precedence
            || (candidate.precedence == current.precedence
                && candidate.occurred_at < current.occurred_at);
        if better {
            *current = candidate;
        }
        current.tool_calls = calls;
    }

    pub fn into_records(self) -> Vec<RawUsage> {
        self.records
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use veyra_domain::time::UtcMillis;

    use super::{plain_components, utc_millis};

    #[test]
    fn accepts_only_plain_components() {
        assert_eq!(
            plain_components(Path::new("a/b.jsonl")),
            Some(vec!["a", "b.jsonl"])
        );
        assert_eq!(plain_components(Path::new("../b.jsonl")), None);
    }

    #[test]
    fn parses_utc_timestamps() {
        assert_eq!(
            utc_millis("1970-01-01T00:00:01.500Z"),
            Some(UtcMillis(1_500))
        );
        assert_eq!(utc_millis("yesterday"), None);
    }
}
