use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const SHODAN_API: &str = "https://api.shodan.io";

pub struct ShodanProvider {
    client: Client,
    api_key: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HostResult {
    ip_str: Option<String>,
    hostnames: Option<Vec<String>>,
    org: Option<String>,
    isp: Option<String>,
    asn: Option<String>,
    country_name: Option<String>,
    city: Option<String>,
    os: Option<String>,
    ports: Option<Vec<u16>>,
    vulns: Option<Vec<String>>,
    tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SearchResult {
    matches: Option<Vec<SearchMatch>>,
    total: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SearchMatch {
    ip_str: String,
    port: u16,
    org: Option<String>,
    hostnames: Option<Vec<String>>,
    product: Option<String>,
    os: Option<String>,
    country_name: Option<String>,
    asn: Option<String>,
}

impl ShodanProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
            api_key: String::new(),
        }
    }

    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
            api_key: api_key.into(),
        }
    }

    fn is_ip_address(query: &str) -> bool {
        query.split('.').count() == 4
            && query.split('.').all(|part| part.parse::<u8>().is_ok())
    }

    fn lookup_host(&self, ip: &str) -> Result<Option<KnowledgeEntry>, String> {
        let url = format!("{}/shodan/host/{}?key={}", SHODAN_API, ip, self.api_key);

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(format!("host lookup failed: {}", response.status()));
        }

        let data: HostResult = response.json().map_err(|e| e.to_string())?;

        let ip_str = data.ip_str.as_deref().unwrap_or(ip);
        let hostnames = data.hostnames.as_ref()
            .map(|h| h.join(", "))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "No hostnames".to_string());

        let location: Vec<&str> = [data.city.as_deref(), data.country_name.as_deref()]
            .into_iter()
            .flatten()
            .collect();

        let ports = data.ports.as_ref()
            .map(|p| p.iter().take(10).map(|n| n.to_string()).collect::<Vec<_>>().join(", "))
            .unwrap_or_else(|| "None".to_string());

        let mut lines = Vec::new();
        lines.push(format!("IP: {}", ip_str));
        if let Some(org) = &data.org {
            lines.push(format!("Organization: {}", org));
        }
        if !location.is_empty() {
            lines.push(format!("Location: {}", location.join(", ")));
        }
        lines.push(format!("Open Ports: {}", ports));
        if let Some(os) = &data.os {
            lines.push(format!("OS: {}", os));
        }
        if let Some(vulns) = &data.vulns {
            if !vulns.is_empty() {
                lines.push(format!("Vulnerabilities: {}", vulns.iter().take(5).cloned().collect::<Vec<_>>().join(", ")));
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("ip".to_string(), serde_json::json!(ip_str));
        if let Some(h) = &data.hostnames {
            metadata.insert("hostnames".to_string(), serde_json::json!(h));
        }
        if let Some(org) = &data.org {
            metadata.insert("org".to_string(), serde_json::json!(org));
        }
        if let Some(p) = &data.ports {
            metadata.insert("ports".to_string(), serde_json::json!(p));
        }
        if let Some(v) = &data.vulns {
            metadata.insert("vulns".to_string(), serde_json::json!(v));
        }

        Ok(Some(KnowledgeEntry {
            title: format!("{} ({})", ip_str, hostnames),
            summary: lines.join("\n"),
            url: Some(format!("https://www.shodan.io/host/{}", ip)),
            source: "shodan".to_string(),
            metadata: Some(metadata),
        }))
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/shodan/host/search?key={}&query={}",
            SHODAN_API,
            self.api_key,
            urlencoding::encode(query)
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("search failed: {}", response.status()));
        }

        let data: SearchResult = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .matches
            .unwrap_or_default()
            .into_iter()
            .take(limit)
            .map(|m| {
                let hostnames = m.hostnames.as_ref()
                    .map(|h| h.join(", "))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "No hostnames".to_string());

                let mut parts = Vec::new();
                parts.push(format!("Port: {}", m.port));
                if let Some(org) = &m.org {
                    parts.push(format!("Org: {}", org));
                }
                if let Some(product) = &m.product {
                    parts.push(format!("Product: {}", product));
                }
                if let Some(country) = &m.country_name {
                    parts.push(format!("Country: {}", country));
                }

                let mut metadata = HashMap::new();
                metadata.insert("ip".to_string(), serde_json::json!(m.ip_str));
                metadata.insert("port".to_string(), serde_json::json!(m.port));
                if let Some(org) = &m.org {
                    metadata.insert("org".to_string(), serde_json::json!(org));
                }
                if let Some(h) = &m.hostnames {
                    metadata.insert("hostnames".to_string(), serde_json::json!(h));
                }

                KnowledgeEntry {
                    title: format!("{}:{} ({})", m.ip_str, m.port, hostnames),
                    summary: parts.join(" | "),
                    url: Some(format!("https://www.shodan.io/host/{}", m.ip_str)),
                    source: "shodan".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }
}

impl Default for ShodanProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for ShodanProvider {
    fn name(&self) -> &'static str {
        "shodan"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

        if self.api_key.is_empty() {
            return LookupResult::error(self.name(), "Shodan API key not configured");
        }

        // Check if query is an IP address
        if Self::is_ip_address(query) {
            match self.lookup_host(query) {
                Ok(Some(entry)) => return LookupResult::success(self.name(), vec![entry]),
                Ok(None) => return LookupResult::success(self.name(), vec![]),
                Err(e) => return LookupResult::error(self.name(), e),
            }
        }

        // Search
        match self.search(query, limit) {
            Ok(entries) => LookupResult::success(self.name(), entries),
            Err(e) => LookupResult::error(self.name(), e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shodan_requires_api_key() {
        let provider = ShodanProvider::new();
        assert!(!provider.is_available());
    }
}
