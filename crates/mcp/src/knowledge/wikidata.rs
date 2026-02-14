use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const WIKIDATA_API: &str = "https://www.wikidata.org/w/api.php";
const WIKIDATA_SPARQL: &str = "https://query.wikidata.org/sparql";

pub struct WikidataProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    search: Option<Vec<SearchItem>>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    id: String,
    label: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SparqlResponse {
    results: Option<SparqlResults>,
}

#[derive(Debug, Deserialize)]
struct SparqlResults {
    bindings: Option<Vec<SparqlBinding>>,
}

#[derive(Debug, Deserialize)]
struct SparqlBinding {
    item: Option<SparqlValue>,
    #[serde(rename = "itemLabel")]
    item_label: Option<SparqlValue>,
    #[serde(rename = "itemDescription")]
    item_description: Option<SparqlValue>,
}

#[derive(Debug, Deserialize)]
struct SparqlValue {
    value: String,
}

impl WikidataProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn is_qid(query: &str) -> bool {
        let trimmed = query.trim();
        trimmed.starts_with('Q') && trimmed[1..].chars().all(|c| c.is_ascii_digit())
    }

    fn search_entities(&self, query: &str, limit: usize, language: &str) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}?action=wbsearchentities&search={}&language={}&limit={}&format=json&origin=*",
            WIKIDATA_API,
            urlencoding::encode(query),
            language,
            limit
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("search failed: {}", response.status()));
        }

        let data: SearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .search
            .unwrap_or_default()
            .into_iter()
            .map(|item| {
                let mut metadata = HashMap::new();
                metadata.insert("qid".to_string(), serde_json::json!(item.id));

                KnowledgeEntry {
                    title: item.label,
                    summary: item.description.unwrap_or_default(),
                    url: Some(format!("https://www.wikidata.org/wiki/{}", item.id)),
                    source: "wikidata".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }

    fn get_entity_by_id(&self, qid: &str, language: &str) -> Result<Option<KnowledgeEntry>, String> {
        let qid_upper = qid.to_uppercase();
        let sparql_query = format!(
            r#"SELECT ?item ?itemLabel ?itemDescription WHERE {{
                BIND(wd:{} AS ?item)
                SERVICE wikibase:label {{ bd:serviceParam wikibase:language "{},en". }}
            }}
            LIMIT 1"#,
            qid_upper, language
        );

        let url = format!(
            "{}?query={}&format=json",
            WIKIDATA_SPARQL,
            urlencoding::encode(&sparql_query)
        );

        let response = self.client
            .get(&url)
            .header("Accept", "application/sparql-results+json")
            .send()
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("SPARQL query failed: {}", response.status()));
        }

        let data: SparqlResponse = response.json().map_err(|e| e.to_string())?;

        let binding = match data.results.and_then(|r| r.bindings).and_then(|b| b.into_iter().next()) {
            Some(b) => b,
            None => return Ok(None),
        };

        let label = match binding.item_label {
            Some(l) => l.value,
            None => return Ok(None),
        };

        let mut metadata = HashMap::new();
        metadata.insert("qid".to_string(), serde_json::json!(qid_upper));

        Ok(Some(KnowledgeEntry {
            title: label,
            summary: binding.item_description.map(|d| d.value).unwrap_or_default(),
            url: binding.item.map(|i| i.value),
            source: "wikidata".to_string(),
            metadata: Some(metadata),
        }))
    }
}

impl Default for WikidataProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for WikidataProvider {
    fn name(&self) -> &'static str {
        "wikidata"
    }

    fn is_available(&self) -> bool {
        self.client
            .get(WIKIDATA_API)
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);
        let language = options.language.as_deref().unwrap_or("en");

        if Self::is_qid(query) {
            match self.get_entity_by_id(query, language) {
                Ok(Some(entry)) => return LookupResult::success(self.name(), vec![entry]),
                Ok(None) => return LookupResult::success(self.name(), vec![]),
                Err(e) => return LookupResult::error(self.name(), e),
            }
        }

        match self.search_entities(query, limit, language) {
            Ok(entries) => LookupResult::success(self.name(), entries),
            Err(e) => LookupResult::error(self.name(), e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires network
    fn wikidata_lookup() {
        let provider = WikidataProvider::new();
        let result = provider.lookup("Q42", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
    }
}
