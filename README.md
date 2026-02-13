# vault-tree

An MCP server that gives Claude Code deep understanding of Obsidian vaults - wikilinks, frontmatter, backlink graphs - without Obsidian running.

## Components

- **vault-tree-core**: Rust library for vault parsing
- **vault-tree-mcp**: Standalone MCP server (stdio)
- **vault-tree-wasm**: WASM bindings for browser/plugin use
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
│   ├── core/               # Shared Rust library
│   │   └── src/
│   │       ├── tree.rs
│   │       ├── frontmatter.rs
│   │       ├── links.rs
│   │       ├── search.rs
│   │       └── fingerprint.rs
│   ├── mcp/                # Standalone MCP server
│   │   └── src/
│   │       ├── main.rs
│   │       ├── server.rs
│   │       └── tools.rs
│   └── wasm/               # WASM bindings
├── plugin/                 # Obsidian plugin
│   ├── src/
│   │   ├── main.ts
│   │   ├── settings.ts
│   │   ├── wasm/
│   │   ├── tree/
│   │   └── mcp/
│   └── wasm/               # Built WASM output
└── README.md
```

## License

MIT
