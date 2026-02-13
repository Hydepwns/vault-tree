use std::path::PathBuf;
use tantivy::{
    collector::TopDocs,
    query::{BooleanQuery, FuzzyTermQuery, Occur, Query, QueryParser},
    schema::Value,
    TantivyDocument, Term,
};
use thiserror::Error;

use super::index::{IndexError, SearchIndex};

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("index error: {0}")]
    Index(#[from] IndexError),
    #[error("query parse error: {0}")]
    Parse(#[from] tantivy::query::QueryParserError),
    #[error("search error: {0}")]
    Search(#[from] tantivy::TantivyError),
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub limit: usize,
    pub snippet_length: usize,
    pub fuzzy: bool,
    pub fuzzy_distance: u8,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 20,
            snippet_length: 150,
            fuzzy: false,
            fuzzy_distance: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_hash: String,
    pub file_path: PathBuf,
    pub title: Option<String>,
    pub author: Option<String>,
    pub score: f32,
    pub snippets: Vec<String>,
}

impl SearchIndex {
    pub fn search(
        &self,
        query_str: &str,
        options: &SearchOptions,
    ) -> Result<Vec<SearchResult>, QueryError> {
        let searcher = self.reader().searcher();
        let schema = self.schema();

        let mut query_parser = QueryParser::for_index(
            self.index(),
            vec![schema.title, schema.author, schema.content],
        );

        // Boost title and author fields for better relevance
        query_parser.set_field_boost(schema.title, 3.0);
        query_parser.set_field_boost(schema.author, 2.0);
        // content has default boost of 1.0

        let query: Box<dyn Query> = if options.fuzzy {
            build_fuzzy_query(query_str, schema, options.fuzzy_distance)
        } else {
            Box::new(query_parser.parse_query(query_str)?)
        };

        let top_docs = searcher.search(&*query, &TopDocs::with_limit(options.limit))?;

        let query_terms: Vec<String> = extract_query_terms(query_str);

        let mut results = Vec::with_capacity(top_docs.len());

        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;

            let file_hash = doc
                .get_first(schema.file_hash)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let file_path = doc
                .get_first(schema.file_path)
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .unwrap_or_default();

            let title = doc
                .get_first(schema.title)
                .and_then(|v| v.as_str())
                .map(String::from);

            let author = doc
                .get_first(schema.author)
                .and_then(|v| v.as_str())
                .map(String::from);

            let content_preview = doc
                .get_first(schema.content_preview)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let snippets = extract_snippets(content_preview, &query_terms, options.snippet_length);

            results.push(SearchResult {
                file_hash,
                file_path,
                title,
                author,
                score,
                snippets,
            });
        }

        Ok(results)
    }
}

fn extract_query_terms(query_str: &str) -> Vec<String> {
    query_str
        .split_whitespace()
        .filter(|s| !matches!(s.to_uppercase().as_str(), "AND" | "OR" | "NOT"))
        .map(|s| {
            s.trim_matches(|c| c == '"' || c == '(' || c == ')')
                .to_lowercase()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn build_fuzzy_query(
    query_str: &str,
    schema: &super::index::SearchSchema,
    distance: u8,
) -> Box<dyn Query> {
    let terms: Vec<&str> = query_str
        .split_whitespace()
        .filter(|s| !matches!(s.to_uppercase().as_str(), "AND" | "OR" | "NOT"))
        .filter(|s| s.len() >= 2)
        .collect();

    if terms.is_empty() {
        return Box::new(tantivy::query::AllQuery);
    }

    let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

    for term in terms {
        let term_lower = term.to_lowercase();

        // Add fuzzy queries for each searchable field
        for (field, boost) in [
            (schema.title, 3.0f32),
            (schema.author, 2.0f32),
            (schema.content, 1.0f32),
        ] {
            let tantivy_term = Term::from_field_text(field, &term_lower);
            let fuzzy_query = FuzzyTermQuery::new(tantivy_term, distance, true);
            let boosted = tantivy::query::BoostQuery::new(Box::new(fuzzy_query), boost);
            subqueries.push((Occur::Should, Box::new(boosted)));
        }
    }

    if subqueries.is_empty() {
        Box::new(tantivy::query::AllQuery)
    } else {
        Box::new(BooleanQuery::new(subqueries))
    }
}

fn extract_snippets(content: &str, query_terms: &[String], max_length: usize) -> Vec<String> {
    if content.is_empty() || query_terms.is_empty() {
        return Vec::new();
    }

    let content_lower = content.to_lowercase();

    for term in query_terms {
        if let Some(pos) = content_lower.find(term) {
            let start = pos.saturating_sub(50);
            let end = (pos + term.len() + 100).min(content.len());

            let mut actual_start = start;
            while actual_start > 0 && !content.is_char_boundary(actual_start) {
                actual_start -= 1;
            }
            let mut actual_end = end;
            while actual_end < content.len() && !content.is_char_boundary(actual_end) {
                actual_end += 1;
            }

            let snippet = &content[actual_start..actual_end];
            let snippet = if actual_start > 0 {
                format!("...{}", snippet)
            } else {
                snippet.to_string()
            };
            let snippet = if actual_end < content.len() {
                format!("{}...", snippet)
            } else {
                snippet
            };

            let truncated = if snippet.len() > max_length {
                let mut trunc_end = max_length;
                while trunc_end < snippet.len() && !snippet.is_char_boundary(trunc_end) {
                    trunc_end += 1;
                }
                format!("{}...", &snippet[..trunc_end])
            } else {
                snippet
            };

            return vec![truncated];
        }
    }

    Vec::new()
}

pub fn format_search_results(results: &[SearchResult], query: &str) -> String {
    if results.is_empty() {
        return format!("No matches found for \"{}\".", query);
    }

    let mut output = format!("Found {} matches for \"{}\":\n\n", results.len(), query);

    for result in results {
        output.push_str(&format!("## {}\n", result.file_path.display()));
        output.push_str(&format!("Score: {:.2}", result.score));

        if let Some(ref title) = result.title {
            output.push_str(&format!(" | Title: {}", title));
        }
        if let Some(ref author) = result.author {
            output.push_str(&format!(" | Author: {}", author));
        }
        output.push('\n');

        for snippet in &result.snippets {
            output.push_str(&format!("\"...{}...\"\n", snippet));
        }

        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn search_returns_results() {
        let temp = TempDir::new().unwrap();
        let mut index = SearchIndex::open_or_create(temp.path()).unwrap();

        index
            .add_document(
                "hash1",
                Path::new("rust/ownership.pdf"),
                Some("Rust Ownership"),
                Some("Steve Klabnik"),
                "The concept of ownership is unique to Rust and enables memory safety.",
            )
            .unwrap();
        index
            .add_document(
                "hash2",
                Path::new("python/guide.pdf"),
                Some("Python Guide"),
                None,
                "Python is a dynamic programming language.",
            )
            .unwrap();
        index.commit().unwrap();

        let options = SearchOptions::default();
        let results = index.search("ownership", &options).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_hash, "hash1");
        assert_eq!(results[0].title, Some("Rust Ownership".to_string()));
    }

    #[test]
    fn search_empty_index() {
        let temp = TempDir::new().unwrap();
        let index = SearchIndex::open_or_create(temp.path()).unwrap();

        let results = index.search("anything", &SearchOptions::default()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn format_empty_results() {
        let output = format_search_results(&[], "test query");
        assert!(output.contains("No matches found"));
    }

    #[test]
    fn format_with_results() {
        let results = vec![SearchResult {
            file_hash: "abc".to_string(),
            file_path: PathBuf::from("test.pdf"),
            title: Some("Test Book".to_string()),
            author: None,
            score: 0.85,
            snippets: vec!["matching text".to_string()],
        }];

        let output = format_search_results(&results, "test");
        assert!(output.contains("Found 1 matches"));
        assert!(output.contains("test.pdf"));
        assert!(output.contains("Test Book"));
    }
}
