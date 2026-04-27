use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FrontMatter {
    pub title: Option<String>,
    pub tags: Tags,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(untagged)]
pub enum Tags {
    List(Vec<String>),
    Csv(String),
    #[default]
    None,
}

pub fn normalize_tags(tags: Tags) -> Vec<String> {
    let raw_tags = match tags {
        Tags::List(tags) => tags,
        Tags::Csv(tags) => tags.split(',').map(ToOwned::to_owned).collect(),
        Tags::None => Vec::new(),
    };

    let mut seen = HashSet::new();
    let mut normalized = raw_tags
        .into_iter()
        .map(|tag| tag.trim().to_owned())
        .filter(|tag| !tag.is_empty())
        .filter(|tag| seen.insert(tag.to_lowercase()))
        .collect::<Vec<_>>();

    normalized.sort_by_key(|tag| tag.to_lowercase());
    normalized
}

#[cfg(test)]
mod tests {
    use super::{Tags, normalize_tags};

    #[test]
    fn normalizes_list_tags() {
        let tags = normalize_tags(Tags::List(vec![
            " GraphRAG ".to_owned(),
            "rust".to_owned(),
            "graphrag".to_owned(),
        ]));

        assert_eq!(tags, vec!["GraphRAG", "rust"]);
    }

    #[test]
    fn normalizes_csv_tags() {
        let tags = normalize_tags(Tags::Csv("rag, graph,  rust ".to_owned()));

        assert_eq!(tags, vec!["graph", "rag", "rust"]);
    }
}
