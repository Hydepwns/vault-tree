use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::types::Topic;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub library_path: PathBuf,
    pub default_topics: Vec<Topic>,
    pub compression: CompressionConfig,
    pub keyword_rules: HashMap<String, Topic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub level: i32,
    pub min_size_bytes: u64,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            level: 3,
            min_size_bytes: 1024 * 1024, // 1MB
        }
    }
}

impl Config {
    pub fn new(library_path: impl Into<PathBuf>) -> Self {
        Self {
            library_path: library_path.into(),
            default_topics: default_topics(),
            compression: CompressionConfig::default(),
            keyword_rules: default_keyword_rules(),
        }
    }

    pub fn topic_path(&self, topic: &Topic) -> PathBuf {
        self.library_path.join(topic.as_str())
    }

    pub fn subtopic_path(&self, topic: &Topic, subtopic: &str) -> PathBuf {
        self.library_path.join(topic.as_str()).join(subtopic)
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.library_path.join("manifest.json")
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(Into::into)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content).map_err(Into::into)
    }
}

fn default_topics() -> Vec<Topic> {
    [
        "programming",
        "electronics",
        "crypto",
        "research",
        "philosophy",
        "mathematics",
        "science",
        "security",
        "manuals",
        "reference",
        "other",
    ]
    .into_iter()
    .map(Topic::new)
    .collect()
}

fn default_keyword_rules() -> HashMap<String, Topic> {
    let topic_keywords: &[(&str, &[&str])] = &[
        (
            "programming",
            &[
                "rust",
                "python",
                "javascript",
                "go",
                "java",
                "programming",
                "software",
                "code",
                "algorithm",
                "compiler",
                "database",
                "web",
                "api",
                "linux",
                "unix",
                "shell",
                "bash",
            ],
        ),
        (
            "electronics",
            &[
                "electronics",
                "circuit",
                "arduino",
                "raspberry",
                "microcontroller",
                "pcb",
                "embedded",
                "fpga",
                "vhdl",
                "verilog",
                "oscilloscope",
                "multimeter",
            ],
        ),
        (
            "philosophy",
            &[
                "philosophy",
                "ethics",
                "metaphysics",
                "epistemology",
                "logic",
                "socrates",
                "plato",
                "aristotle",
                "kant",
                "nietzsche",
                "existentialism",
            ],
        ),
        (
            "mathematics",
            &[
                "mathematics",
                "calculus",
                "algebra",
                "geometry",
                "statistics",
                "probability",
                "linear",
                "discrete",
                "analysis",
                "theorem",
            ],
        ),
        (
            "science",
            &[
                "physics",
                "chemistry",
                "biology",
                "quantum",
                "relativity",
                "thermodynamics",
                "organic",
                "biochemistry",
                "neuroscience",
            ],
        ),
        (
            "crypto",
            &[
                "bitcoin",
                "ethereum",
                "blockchain",
                "crypto",
                "cryptocurrency",
                "defi",
                "nft",
                "token",
                "wallet",
                "consensus",
                "staking",
                "validator",
                "solidity",
                "smartcontract",
                "web3",
                "dapp",
                "dao",
                "amm",
                "uniswap",
                "aave",
                "lido",
                "eigenlayer",
                "rollup",
                "zk",
                "zkp",
                "snark",
                "stark",
                "merkle",
                "hash",
                "evm",
                "cosmos",
                "polkadot",
                "solana",
                "avalanche",
                "arbitrum",
                "optimism",
                "layer2",
                "bridge",
                "oracle",
                "chainlink",
                "whitepaper",
            ],
        ),
        (
            "security",
            &[
                "security",
                "cybersecurity",
                "hacking",
                "penetration",
                "pentest",
                "exploit",
                "vulnerability",
                "malware",
                "forensics",
                "incident",
                "redteam",
                "blueteam",
                "audit",
                "encryption",
                "cryptography",
            ],
        ),
        (
            "research",
            &[
                "arxiv",
                "paper",
                "thesis",
                "dissertation",
                "journal",
                "conference",
                "proceedings",
                "preprint",
                "manuscript",
                "survey",
                "review",
                "abstract",
            ],
        ),
    ];

    topic_keywords
        .iter()
        .flat_map(|(topic, keywords)| {
            let topic = Topic::new(*topic);
            keywords
                .iter()
                .map(move |&kw| (kw.to_string(), topic.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn config_paths() {
        let config = Config::new("/lib");
        assert_eq!(
            config.topic_path(&Topic::new("programming")),
            PathBuf::from("/lib/programming")
        );
        assert_eq!(
            config.subtopic_path(&Topic::new("programming"), "rust"),
            PathBuf::from("/lib/programming/rust")
        );
        assert_eq!(config.manifest_path(), PathBuf::from("/lib/manifest.json"));
    }

    #[test]
    fn config_save_load() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.json");

        let config = Config::new("/lib");
        config.save(&config_path).unwrap();

        let loaded = Config::load(&config_path).unwrap();
        assert_eq!(loaded.library_path, config.library_path);
    }

    #[test]
    fn keyword_rules_built_functionally() {
        let rules = default_keyword_rules();

        assert_eq!(rules.get("rust"), Some(&Topic::new("programming")));
        assert_eq!(rules.get("arduino"), Some(&Topic::new("electronics")));
        assert_eq!(rules.get("plato"), Some(&Topic::new("philosophy")));
        assert_eq!(rules.get("calculus"), Some(&Topic::new("mathematics")));
        assert_eq!(rules.get("quantum"), Some(&Topic::new("science")));
        assert_eq!(rules.get("bitcoin"), Some(&Topic::new("crypto")));
        assert_eq!(rules.get("ethereum"), Some(&Topic::new("crypto")));
        assert_eq!(rules.get("eigenlayer"), Some(&Topic::new("crypto")));
        assert_eq!(rules.get("cybersecurity"), Some(&Topic::new("security")));
        assert_eq!(rules.get("pentest"), Some(&Topic::new("security")));
    }
}
