# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build --release           # Build all crates
cargo test                      # Run all tests
cargo test -p vault-tree-mcp    # Test specific crate
cargo test test_name            # Run single test
cargo fmt                       # Format all code
cargo clippy                    # Lint all code

# WASM (for Obsidian plugin)
cd crates/wasm && wasm-pack build --target web --out-dir ../../plugin/wasm

# Plugin
cd plugin && npm install && npm run build
```

## Architecture

Rust workspace with three crates plus an Obsidian plugin.

### vault-tree-core
Shared library for Obsidian vault parsing (tree.rs, frontmatter.rs, links.rs, search.rs, fingerprint.rs).

### vault-tree-mcp
Standalone MCP server (JSON-RPC over stdio):
- `server.rs` - MCP protocol handler
- `tools/` - Modular tool implementations (vault, knowledge)
- `knowledge/` - 15 external data providers with LRU caching
- `transport.rs` - JSON-RPC types

### vault-tree-wasm
WASM bindings for browser/Obsidian plugin use.

## Related Projects

- **packup** (`/Users/droo/Documents/CODE/packup`) - Document library organizer with BLAKE3 dedup, classification, and MinIO sync. Previously `lib-organizer` in this workspace.

## Knowledge Providers

15 providers in `crates/mcp/src/knowledge/`:
- General: wikipedia, dbpedia, wikidata
- Code: github, sourceforge, npm, crates.io, stackoverflow
- Social: reddit
- Reference: openlibrary, arxiv, musicbrainz, wikiart
- Specialized: defillama, shodan

Registry uses LRU cache (100 items, 15min TTL). Auto-lookup tries providers in PROVIDER_ORDER.

## Environment Variables

- `GITHUB_TOKEN` - Higher rate limits (5000/hr vs 60/hr)
- `SHODAN_API_KEY` - Required for Shodan provider

## MCP Tools

- `vault_tree`, `vault_search` - Vault operations
- `knowledge_lookup` - External knowledge lookups

## Testing MCP Server

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | ./target/release/vault-tree-mcp
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | ./target/release/vault-tree-mcp
```

## Code Style

- Use `thiserror` for library errors, `anyhow` for applications
- Implement `Display` for user-facing types (not Debug format in output)
