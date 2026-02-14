use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const DEFILLAMA_API: &str = "https://api.llama.fi";

pub struct DefiLlamaProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Protocol {
    id: String,
    name: String,
    slug: String,
    symbol: Option<String>,
    url: Option<String>,
    description: Option<String>,
    chain: Option<String>,
    chains: Option<Vec<String>>,
    tvl: Option<f64>,
    change_1h: Option<f64>,
    change_1d: Option<f64>,
    change_7d: Option<f64>,
    category: Option<String>,
    logo: Option<String>,
    twitter: Option<String>,
    gecko_id: Option<String>,
    mcap: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Chain {
    name: String,
    tvl: f64,
    #[serde(rename = "chainId")]
    chain_id: Option<i64>,
    gecko_id: Option<String>,
    #[serde(rename = "tokenSymbol")]
    token_symbol: Option<String>,
}

impl DefiLlamaProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn format_tvl(tvl: Option<f64>) -> String {
        match tvl {
            None => "N/A".to_string(),
            Some(v) if v >= 1e9 => format!("${:.2}B", v / 1e9),
            Some(v) if v >= 1e6 => format!("${:.2}M", v / 1e6),
            Some(v) if v >= 1e3 => format!("${:.2}K", v / 1e3),
            Some(v) => format!("${:.2}", v),
        }
    }

    fn protocol_to_entry(&self, protocol: &Protocol) -> KnowledgeEntry {
        let mut lines = Vec::new();

        if let Some(desc) = &protocol.description {
            lines.push(desc.chars().take(200).collect::<String>());
        }

        let tvl = Self::format_tvl(protocol.tvl);
        lines.push(format!("TVL: {}", tvl));

        if let Some(cat) = &protocol.category {
            lines.push(format!("Category: {}", cat));
        }

        if let Some(chains) = &protocol.chains {
            if !chains.is_empty() {
                let chain_list: Vec<_> = chains.iter().take(5).cloned().collect();
                lines.push(format!("Chains: {}", chain_list.join(", ")));
            }
        }

        if let Some(change) = protocol.change_1d {
            let sign = if change >= 0.0 { "+" } else { "" };
            lines.push(format!("24h Change: {}{:.2}%", sign, change));
        }

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), serde_json::json!("protocol"));
        metadata.insert("id".to_string(), serde_json::json!(protocol.id));
        metadata.insert("slug".to_string(), serde_json::json!(protocol.slug));
        if let Some(sym) = &protocol.symbol {
            metadata.insert("symbol".to_string(), serde_json::json!(sym));
        }
        if let Some(cat) = &protocol.category {
            metadata.insert("category".to_string(), serde_json::json!(cat));
        }
        if let Some(tvl) = protocol.tvl {
            metadata.insert("tvl".to_string(), serde_json::json!(tvl));
        }
        if let Some(chains) = &protocol.chains {
            metadata.insert("chains".to_string(), serde_json::json!(chains));
        }

        KnowledgeEntry {
            title: protocol.name.clone(),
            summary: lines.join("\n"),
            url: Some(format!("https://defillama.com/protocol/{}", protocol.slug)),
            source: "defillama".to_string(),
            metadata: Some(metadata),
        }
    }

    fn search_protocols(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!("{}/protocols", DEFILLAMA_API);

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("failed to fetch protocols: {}", response.status()));
        }

        let protocols: Vec<Protocol> = response.json().map_err(|e| e.to_string())?;

        let query_lower = query.to_lowercase();

        let mut matches: Vec<_> = protocols
            .iter()
            .filter(|p| {
                let name_match = p.name.to_lowercase().contains(&query_lower);
                let symbol_match = p.symbol.as_ref().map(|s| s.to_lowercase().contains(&query_lower)).unwrap_or(false);
                let category_match = p.category.as_ref().map(|c| c.to_lowercase().contains(&query_lower)).unwrap_or(false);
                name_match || symbol_match || category_match
            })
            .collect();

        matches.sort_by(|a, b| {
            let a_exact = a.name.to_lowercase() == query_lower;
            let b_exact = b.name.to_lowercase() == query_lower;
            if a_exact != b_exact {
                return b_exact.cmp(&a_exact);
            }
            b.tvl.partial_cmp(&a.tvl).unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(matches
            .into_iter()
            .take(limit)
            .map(|p| self.protocol_to_entry(p))
            .collect())
    }

    fn lookup_chain(&self, name: &str) -> Result<Option<KnowledgeEntry>, String> {
        let url = format!("{}/v2/chains", DEFILLAMA_API);

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let chains: Vec<Chain> = response.json().map_err(|e| e.to_string())?;

        let name_lower = name.to_lowercase();
        let chain = chains.iter().find(|c| c.name.to_lowercase() == name_lower);

        match chain {
            None => Ok(None),
            Some(c) => {
                let tvl = Self::format_tvl(Some(c.tvl));

                let mut metadata = HashMap::new();
                metadata.insert("type".to_string(), serde_json::json!("chain"));
                metadata.insert("tvl".to_string(), serde_json::json!(c.tvl));
                metadata.insert("tvlFormatted".to_string(), serde_json::json!(tvl));
                if let Some(id) = c.chain_id {
                    metadata.insert("chainId".to_string(), serde_json::json!(id));
                }
                if let Some(sym) = &c.token_symbol {
                    metadata.insert("tokenSymbol".to_string(), serde_json::json!(sym));
                }

                Ok(Some(KnowledgeEntry {
                    title: c.name.clone(),
                    summary: format!("Blockchain with {} TVL", tvl),
                    url: Some(format!("https://defillama.com/chain/{}", c.name)),
                    source: "defillama".to_string(),
                    metadata: Some(metadata),
                }))
            }
        }
    }
}

impl Default for DefiLlamaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for DefiLlamaProvider {
    fn name(&self) -> &'static str {
        "defillama"
    }

    fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/protocols", DEFILLAMA_API))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

        // Check if query is a chain name
        if let Ok(Some(entry)) = self.lookup_chain(query) {
            return LookupResult::success(self.name(), vec![entry]);
        }

        // Search protocols
        match self.search_protocols(query, limit) {
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
    fn defillama_lookup() {
        let provider = DefiLlamaProvider::new();
        let result = provider.lookup("uniswap", &LookupOptions::default());
        assert!(result.success);
    }
}
