use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const OPENLIBRARY_API: &str = "https://openlibrary.org";

pub struct OpenLibraryProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct BookSearchResponse {
    docs: Option<Vec<BookDoc>>,
}

#[derive(Debug, Deserialize)]
struct BookDoc {
    key: String,
    title: String,
    author_name: Option<Vec<String>>,
    first_publish_year: Option<i32>,
    isbn: Option<Vec<String>>,
    subject: Option<Vec<String>>,
    cover_i: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AuthorSearchResponse {
    docs: Option<Vec<AuthorDoc>>,
}

#[derive(Debug, Deserialize)]
struct AuthorDoc {
    key: String,
    name: String,
    birth_date: Option<String>,
    death_date: Option<String>,
    top_work: Option<String>,
    work_count: Option<i32>,
}

impl OpenLibraryProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn search_books(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/search.json?q={}&limit={}&fields=key,title,author_name,first_publish_year,isbn,subject,cover_i",
            OPENLIBRARY_API,
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let data: BookSearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .docs
            .unwrap_or_default()
            .into_iter()
            .map(|book| {
                let authors = book.author_name.as_ref().map(|a| a.join(", ")).unwrap_or_else(|| "Unknown author".to_string());
                let year = book.first_publish_year.map(|y| format!(" ({})", y)).unwrap_or_default();

                let mut metadata = HashMap::new();
                metadata.insert("type".to_string(), serde_json::json!("book"));
                if let Some(ref author_list) = book.author_name {
                    metadata.insert("authors".to_string(), serde_json::json!(author_list));
                }
                if let Some(y) = book.first_publish_year {
                    metadata.insert("year".to_string(), serde_json::json!(y));
                }
                if let Some(ref isbns) = book.isbn {
                    if let Some(first_isbn) = isbns.first() {
                        metadata.insert("isbn".to_string(), serde_json::json!(first_isbn));
                    }
                }
                if let Some(ref subjects) = book.subject {
                    let limited: Vec<_> = subjects.iter().take(5).cloned().collect();
                    metadata.insert("subjects".to_string(), serde_json::json!(limited));
                }
                if let Some(cover_id) = book.cover_i {
                    metadata.insert("coverId".to_string(), serde_json::json!(cover_id));
                }

                KnowledgeEntry {
                    title: book.title,
                    summary: format!("{}{}", authors, year),
                    url: Some(format!("https://openlibrary.org{}", book.key)),
                    source: "openlibrary".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }

    fn search_authors(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/search/authors.json?q={}&limit={}",
            OPENLIBRARY_API,
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let data: AuthorSearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .docs
            .unwrap_or_default()
            .into_iter()
            .map(|author| {
                let years = match (&author.birth_date, &author.death_date) {
                    (Some(b), Some(d)) => format!(" ({} - {})", b, d),
                    (Some(b), None) => format!(" ({} - )", b),
                    (None, Some(d)) => format!(" (? - {})", d),
                    (None, None) => String::new(),
                };
                let works = author.work_count.map(|c| format!(", {} works", c)).unwrap_or_default();
                let top_work = author.top_work.as_ref().map(|w| format!(". Notable: \"{}\"", w)).unwrap_or_default();

                let mut metadata = HashMap::new();
                metadata.insert("type".to_string(), serde_json::json!("author"));
                if let Some(ref b) = author.birth_date {
                    metadata.insert("birthDate".to_string(), serde_json::json!(b));
                }
                if let Some(ref d) = author.death_date {
                    metadata.insert("deathDate".to_string(), serde_json::json!(d));
                }
                if let Some(c) = author.work_count {
                    metadata.insert("workCount".to_string(), serde_json::json!(c));
                }
                if let Some(ref w) = author.top_work {
                    metadata.insert("topWork".to_string(), serde_json::json!(w));
                }

                let author_key = author.key.trim_start_matches("/authors/");

                KnowledgeEntry {
                    title: author.name,
                    summary: format!("Author{}{}{}", years, works, top_work),
                    url: Some(format!("https://openlibrary.org/authors/{}", author_key)),
                    source: "openlibrary".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }
}

impl Default for OpenLibraryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for OpenLibraryProvider {
    fn name(&self) -> &'static str {
        "openlibrary"
    }

    fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/search.json?q=test&limit=1", OPENLIBRARY_API))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

        let mut entries = match self.search_books(query, limit) {
            Ok(e) => e,
            Err(e) => return LookupResult::error(self.name(), e),
        };

        if entries.len() < limit {
            let remaining = limit - entries.len();
            match self.search_authors(query, remaining) {
                Ok(authors) => entries.extend(authors),
                Err(e) => return LookupResult::error(self.name(), e),
            }
        }

        LookupResult::success(self.name(), entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires network
    fn openlibrary_lookup() {
        let provider = OpenLibraryProvider::new();
        let result = provider.lookup("dune frank herbert", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
    }
}
