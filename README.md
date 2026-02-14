# vault-tree

An MCP server that gives Claude Code deep understanding of Obsidian vaults - wikilinks, frontmatter, backlink graphs - without Obsidian running.

## Components

- **vault-tree-core**: Rust library for vault parsing
- **vault-tree-mcp**: Standalone MCP server (stdio)
- **vault-tree-wasm**: WASM bindings for browser/plugin use
- **lib-organizer**: Document library organizer with CLI and full-text search
- **plugin**: Obsidian plugin with embedded WASM and MCP server

## Quick Start

### Standalone Binary

```bash
cargo build --release -p vault-tree-mcp
cp ./target/release/vault-tree-mcp ~/.local/bin/
```

Add to your Claude Code MCP config:

```json
{
  "mcpServers": {
    "vault-tree": {
      "command": "vault-tree-mcp"
    }
  }
}
```

### Obsidian Plugin

Build the WASM module:
```bash
cd crates/wasm && wasm-pack build --target web --out-dir ../../plugin/wasm
```

Build the plugin:
```bash
cd plugin && npm install && npm run build
```

Copy `plugin/` contents to `.obsidian/plugins/vault-tree/` and enable in Obsidian settings.

The plugin runs an MCP server on port 22365 (WebSocket for Claude Code CLI) and 22366 (HTTP for Claude Desktop).

## Tools

### Knowledge Providers

Look up external information from FOSS knowledge bases:

| Provider | Type | Data |
|----------|------|------|
| `wikipedia` | Encyclopedia | Articles, summaries |
| `dbpedia` | Structured data | Wikipedia as linked data |
| `wikidata` | Knowledge graph | Entities, relations, QIDs |
| `github` | Code | Repos, users, stars (token optional) |
| `sourceforge` | Code | Projects, downloads |
| `npm` | Packages | Node.js packages, versions |
| `crates.io` | Packages | Rust crates, downloads |
| `stackoverflow` | Q&A | Questions, answers, tags |
| `reddit` | Social | Posts, subreddits, scores |
| `openlibrary` | Books | Authors, works, ISBNs |
| `arxiv` | Academic | Papers, authors, abstracts |
| `musicbrainz` | Music | Artists, albums, tracks |
| `wikiart` | Art | Artists, paintings |
| `defillama` | DeFi/Crypto | Protocols, TVL, chains |
| `shodan` | Security | IPs, ports, vulns (needs API key) |

**Environment variables:**
- `GITHUB_TOKEN` - GitHub PAT for higher rate limits (5000/hr vs 60/hr)
- `SHODAN_API_KEY` - Required for Shodan provider

```json
{
  "name": "knowledge_lookup",
  "arguments": {
    "query": "Functional programming",
    "provider": "wikipedia"
  }
}
```

### AI Link Suggestions

Use local (Ollama) or cloud (OpenAI/OpenRouter) LLMs to suggest internal links:

```json
{
  "name": "suggest_links",
  "arguments": {
    "file_path": "path/to/note.md",
    "apply": true
  }
}
```

### Batch Processing

Process entire folders:

```json
{
  "name": "batch_suggest_links",
  "arguments": {
    "folder_path": "Writing",
    "apply": true
  }
}
```

### Note Generation

Create notes from knowledge lookups:

```json
{
  "name": "create_note",
  "arguments": {
    "query": "Uniswap",
    "provider": "defillama"
  }
}
```

### vault_tree

Generates an annotated tree of your vault:

```json
{
  "vault_path": "/path/to/vault",
  "depth": 3
}
```

Output is designed to be token-efficient:

```
Writing/
|-- building-droo-foo.md  [elixir,phoenix] 2025-01-18 <-3 ->7
|-- the-agalma.md         [philosophy] 2025-11-14 <-1 ->2
`-- drafts/
    `-- obsidian-plugin.md  [draft] <-0 ->0

3 notes, 2 directories
```

Tags and dates come from frontmatter. `<-N` is incoming backlinks, `->N` is outgoing links.

### vault_search

Regex search across all markdown files:

```json
{
  "vault_path": "/path/to/vault",
  "pattern": "TODO",
  "case_insensitive": true,
  "max_results": 50
}
```

### Library Tools

Manage a document library (PDF, EPUB, DJVU, MOBI):

| Tool | Description |
|------|-------------|
| `lib_init` | Initialize a new library with git tracking |
| `lib_scan` | Scan directories for documents |
| `lib_duplicates` | Find duplicate files by BLAKE3 hash |
| `lib_classify` | Get topic suggestions from filename |
| `lib_ingest` | Add files to library with auto-classification |
| `lib_search` | Search library metadata |
| `lib_pdf_search` | Full-text search in PDF/EPUB content (Tantivy) |
| `lib_status` | Show library statistics |
| `secrets_scan` | Detect sensitive files (keys, credentials) |

### lib-organizer CLI

```bash
lib-organizer init ~/library              # Create library
lib-organizer scan -d ~/Downloads         # Find documents
lib-organizer ingest -l ~/library *.pdf   # Add to library
lib-organizer search -l ~/library "rust"  # Search metadata
lib-organizer search -l ~/library --fulltext "ownership"  # Full-text search
lib-organizer secrets ~/projects --strict # Scan for secrets
lib-organizer completions zsh             # Generate shell completions
```

## Plugin Commands

- **Copy vault tree to clipboard**
- **Insert vault tree at cursor** (as code block)
- **Show vault tree in modal**

## Building

```bash
cargo build --release    # all Rust crates
cargo test               # run tests

# WASM
cd crates/wasm && wasm-pack build --target web --out-dir ../../plugin/wasm

# Plugin
cd plugin && npm run build
```

## Testing the MCP Server

```bash
# Initialize
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | ./target/release/vault-tree-mcp

# List tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | ./target/release/vault-tree-mcp

# Generate tree
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"vault_tree","arguments":{"vault_path":"/path/to/vault"}}}' | ./target/release/vault-tree-mcp
```

## Structure

```
vault-tree/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── core/               # Vault parsing library
│   ├── mcp/                # MCP server
│   ├── lib-organizer/      # Document library + CLI
│   │   └── src/
│   │       ├── cli.rs      # Binary entry point
│   │       ├── scanner.rs  # File discovery
│   │       ├── classifier.rs
│   │       ├── organizer.rs
│   │       ├── search/     # Tantivy full-text search
│   │       └── secrets.rs
│   └── wasm/               # WASM bindings
├── plugin/                 # Obsidian plugin
└── README.md
```

## License

MIT
