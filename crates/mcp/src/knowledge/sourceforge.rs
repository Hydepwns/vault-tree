use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const SOURCEFORGE_API: &str = "https://sourceforge.net/api";

pub struct SourceForgeProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct ProjectResponse {
    #[serde(rename = "Project")]
    project: Option<Project>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Project {
    name: String,
    shortname: String,
    summary: Option<String>,
    description: Option<String>,
    url: Option<String>,
    created: Option<String>,
    homepage: Option<String>,
    external_homepage: Option<String>,
    download_page: Option<String>,
    categories: Option<Categories>,
}

#[derive(Debug, Deserialize)]
struct Categories {
    topic: Option<Vec<Category>>,
    os: Option<Vec<Category>>,
    language: Option<Vec<Category>>,
    license: Option<Vec<Category>>,
}

#[derive(Debug, Deserialize)]
struct Category {
    fullname: String,
}

impl SourceForgeProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn project_to_entry(&self, project: &Project) -> KnowledgeEntry {
        let mut lines = Vec::new();

        if let Some(summary) = &project.summary {
            lines.push(summary.clone());
        }

        if let Some(cats) = &project.categories {
            if let Some(langs) = &cats.language {
                let lang_names: Vec<_> = langs.iter().take(3).map(|c| c.fullname.clone()).collect();
                if !lang_names.is_empty() {
                    lines.push(format!("Languages: {}", lang_names.join(", ")));
                }
            }

            if let Some(licenses) = &cats.license {
                let lic_names: Vec<_> = licenses.iter().take(2).map(|c| c.fullname.clone()).collect();
                if !lic_names.is_empty() {
                    lines.push(format!("License: {}", lic_names.join(", ")));
                }
            }

            if let Some(os) = &cats.os {
                let os_names: Vec<_> = os.iter().take(3).map(|c| c.fullname.clone()).collect();
                if !os_names.is_empty() {
                    lines.push(format!("Platforms: {}", os_names.join(", ")));
                }
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("shortname".to_string(), serde_json::json!(project.shortname));
        if let Some(desc) = &project.description {
            metadata.insert("description".to_string(), serde_json::json!(desc));
        }
        if let Some(hp) = project.external_homepage.as_ref().or(project.homepage.as_ref()) {
            metadata.insert("homepage".to_string(), serde_json::json!(hp));
        }
        if let Some(cats) = &project.categories {
            if let Some(topics) = &cats.topic {
                let names: Vec<_> = topics.iter().map(|t| t.fullname.clone()).collect();
                metadata.insert("topics".to_string(), serde_json::json!(names));
            }
            if let Some(langs) = &cats.language {
                let names: Vec<_> = langs.iter().map(|l| l.fullname.clone()).collect();
                metadata.insert("languages".to_string(), serde_json::json!(names));
            }
        }

        let url = project.url.clone()
            .unwrap_or_else(|| format!("https://sourceforge.net/projects/{}/", project.shortname));

        KnowledgeEntry {
            title: project.name.clone(),
            summary: lines.join("\n"),
            url: Some(url),
            source: "sourceforge".to_string(),
            metadata: Some(metadata),
        }
    }

    fn lookup_project(&self, name: &str) -> Result<Option<KnowledgeEntry>, String> {
        let url = format!("{}/project/name/{}/json", SOURCEFORGE_API, urlencoding::encode(name));

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(format!("project lookup failed: {}", response.status()));
        }

        let data: ProjectResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data.project.as_ref().map(|p| self.project_to_entry(p)))
    }
}

impl Default for SourceForgeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for SourceForgeProvider {
    fn name(&self) -> &'static str {
        "sourceforge"
    }

    fn is_available(&self) -> bool {
        self.client
            .get(&format!("{}/project/name/test/json", SOURCEFORGE_API))
            .send()
            .map(|r| r.status().is_success() || r.status().as_u16() == 404)
            .unwrap_or(false)
    }

    fn lookup(&self, query: &str, _options: &LookupOptions) -> LookupResult {
        // Try direct project lookup
        match self.lookup_project(query) {
            Ok(Some(entry)) => LookupResult::success(self.name(), vec![entry]),
            Ok(None) => {
                // Try with spaces replaced
                let normalized = query.replace(' ', "").to_lowercase();
                match self.lookup_project(&normalized) {
                    Ok(Some(entry)) => LookupResult::success(self.name(), vec![entry]),
                    Ok(None) => LookupResult::success(self.name(), vec![]),
                    Err(e) => LookupResult::error(self.name(), e),
                }
            }
            Err(e) => LookupResult::error(self.name(), e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires network
    fn sourceforge_lookup() {
        let provider = SourceForgeProvider::new();
        let result = provider.lookup("audacity", &LookupOptions::default());
        assert!(result.success);
    }
}
