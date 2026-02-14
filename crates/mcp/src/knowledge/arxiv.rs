use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::blocking::Client;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const ARXIV_API: &str = "https://export.arxiv.org/api/query";

pub struct ArxivProvider {
    client: Client,
}

#[derive(Debug, Default)]
struct ArxivEntry {
    id: String,
    title: String,
    summary: String,
    authors: Vec<String>,
    published: String,
    updated: String,
    categories: Vec<String>,
    pdf_link: Option<String>,
    doi: Option<String>,
}

impl ArxivProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn parse_atom_feed(&self, xml: &str) -> Vec<ArxivEntry> {
        let mut entries = Vec::new();
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut in_entry = false;
        let mut in_author = false;
        let mut current_entry = ArxivEntry::default();
        let mut current_tag = String::new();
        let mut current_author_name = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    current_tag = tag_name.clone();

                    match tag_name.as_str() {
                        "entry" => {
                            in_entry = true;
                            current_entry = ArxivEntry::default();
                        }
                        "author" if in_entry => {
                            in_author = true;
                            current_author_name.clear();
                        }
                        "link" if in_entry => {
                            let mut is_pdf = false;
                            let mut href = String::new();

                            for attr in e.attributes().flatten() {
                                let key = String::from_utf8_lossy(attr.key.as_ref());
                                let val = String::from_utf8_lossy(&attr.value);
                                if key == "title" && val == "pdf" {
                                    is_pdf = true;
                                }
                                if key == "href" {
                                    href = val.to_string();
                                }
                            }

                            if is_pdf && !href.is_empty() {
                                current_entry.pdf_link = Some(href);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(e)) if in_entry => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    match tag_name.as_str() {
                        "link" => {
                            let mut is_pdf = false;
                            let mut href = String::new();

                            for attr in e.attributes().flatten() {
                                let key = String::from_utf8_lossy(attr.key.as_ref());
                                let val = String::from_utf8_lossy(&attr.value);
                                if key == "title" && val == "pdf" {
                                    is_pdf = true;
                                }
                                if key == "href" {
                                    href = val.to_string();
                                }
                            }

                            if is_pdf && !href.is_empty() {
                                current_entry.pdf_link = Some(href);
                            }
                        }
                        "category" => {
                            for attr in e.attributes().flatten() {
                                let key = String::from_utf8_lossy(attr.key.as_ref());
                                if key == "term" {
                                    let val = String::from_utf8_lossy(&attr.value).to_string();
                                    current_entry.categories.push(val);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    match tag_name.as_str() {
                        "entry" => {
                            if !current_entry.id.is_empty() && !current_entry.title.is_empty() {
                                entries.push(std::mem::take(&mut current_entry));
                            }
                            in_entry = false;
                        }
                        "author" if in_entry => {
                            if !current_author_name.is_empty() {
                                current_entry.authors.push(std::mem::take(&mut current_author_name));
                            }
                            in_author = false;
                        }
                        _ => {}
                    }
                    current_tag.clear();
                }
                Ok(Event::Text(e)) => {
                    let text = e.unescape().map(|s| s.trim().to_string()).unwrap_or_default();
                    if !text.is_empty() && in_entry {
                        match current_tag.as_str() {
                            "id" => current_entry.id = text,
                            "title" => {
                                current_entry.title = text.split_whitespace().collect::<Vec<_>>().join(" ");
                            }
                            "summary" => {
                                current_entry.summary = text.split_whitespace().collect::<Vec<_>>().join(" ");
                            }
                            "published" => current_entry.published = text,
                            "updated" => current_entry.updated = text,
                            "name" if in_author => current_author_name = text,
                            "arxiv:doi" => current_entry.doi = Some(text),
                            _ => {}
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        entries
    }

    fn extract_arxiv_id(url: &str) -> String {
        url.rsplit("/abs/")
            .next()
            .map(String::from)
            .unwrap_or_else(|| url.to_string())
    }
}

impl Default for ArxivProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for ArxivProvider {
    fn name(&self) -> &'static str {
        "arxiv"
    }

    fn is_available(&self) -> bool {
        self.client
            .get(format!("{}?search_query=all:test&max_results=1", ARXIV_API))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

        let url = format!(
            "{}?search_query=all:{}&start=0&max_results={}&sortBy=relevance&sortOrder=descending",
            ARXIV_API,
            urlencoding::encode(query),
            limit
        );

        let response = match self.client.get(&url).send() {
            Ok(r) => r,
            Err(e) => return LookupResult::error(self.name(), e.to_string()),
        };

        if !response.status().is_success() {
            return LookupResult::error(
                self.name(),
                format!("arxiv request failed: {}", response.status()),
            );
        }

        let xml = match response.text() {
            Ok(t) => t,
            Err(e) => return LookupResult::error(self.name(), e.to_string()),
        };

        let arxiv_entries = self.parse_atom_feed(&xml);

        let entries: Vec<KnowledgeEntry> = arxiv_entries
            .into_iter()
            .map(|entry| {
                let year = entry.published.get(..4).unwrap_or("????");
                let author_list = if entry.authors.len() > 3 {
                    format!("{} et al.", entry.authors[..3].join(", "))
                } else {
                    entry.authors.join(", ")
                };

                let summary_text = if entry.summary.len() > 400 {
                    format!("{}...", &entry.summary[..400])
                } else {
                    entry.summary.clone()
                };

                let summary = format!("{} ({})\n\n{}", author_list, year, summary_text);

                let mut metadata = HashMap::new();
                metadata.insert("authors".to_string(), serde_json::json!(entry.authors));
                metadata.insert("published".to_string(), serde_json::json!(entry.published));
                metadata.insert("updated".to_string(), serde_json::json!(entry.updated));
                metadata.insert("categories".to_string(), serde_json::json!(entry.categories));
                metadata.insert(
                    "arxivId".to_string(),
                    serde_json::json!(Self::extract_arxiv_id(&entry.id)),
                );
                if let Some(pdf) = &entry.pdf_link {
                    metadata.insert("pdfLink".to_string(), serde_json::json!(pdf));
                }
                if let Some(doi) = &entry.doi {
                    metadata.insert("doi".to_string(), serde_json::json!(doi));
                }

                KnowledgeEntry {
                    title: entry.title,
                    summary,
                    url: Some(entry.id),
                    source: "arxiv".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect();

        LookupResult::success(self.name(), entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires network
    fn arxiv_lookup() {
        let provider = ArxivProvider::new();
        let result = provider.lookup("transformer neural network", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
    }

    #[test]
    fn parse_arxiv_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <id>http://arxiv.org/abs/2301.12345v1</id>
    <title>Test Paper Title</title>
    <summary>This is a test abstract.</summary>
    <published>2023-01-15T00:00:00Z</published>
    <updated>2023-01-16T00:00:00Z</updated>
    <author><name>John Doe</name></author>
    <author><name>Jane Smith</name></author>
    <category term="cs.LG"/>
    <link title="pdf" href="http://arxiv.org/pdf/2301.12345v1"/>
  </entry>
</feed>"#;

        let provider = ArxivProvider::new();
        let entries = provider.parse_atom_feed(xml);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Test Paper Title");
        assert_eq!(entries[0].authors, vec!["John Doe", "Jane Smith"]);
        assert_eq!(entries[0].categories, vec!["cs.LG"]);
        assert!(entries[0].pdf_link.is_some());
    }
}
