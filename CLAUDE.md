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

Rust workspace with four crates plus an Obsidian plugin.

### vault-tree-core
Shared library for Obsidian vault parsing (tree.rs, frontmatter.rs, links.rs, search.rs, fingerprint.rs).

### vault-tree-mcp
Standalone MCP server (JSON-RPC over stdio):
- `server.rs` - MCP protocol handler
- `tools/` - Modular tool implementations (vault, library, knowledge, secrets)
- `knowledge/` - 15 external data providers with LRU caching
- `transport.rs` - JSON-RPC types

### lib-organizer
Document library organizer with CLI (scanner, classifier, organizer, Tantivy search, secrets detection, MinIO sync).

### vault-tree-wasm
WASM bindings for browser/Obsidian plugin use.

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
- `MINIO_ENDPOINT` - MinIO server URL for lib-organizer sync
- `MINIO_ACCESS_KEY` - MinIO access key for lib-organizer sync
- `MINIO_SECRET_KEY` - MinIO secret key for lib-organizer sync

## MCP Tools

- `vault_tree`, `vault_search` - Vault operations
- `knowledge_lookup` - External knowledge lookups
- `lib_scan`, `lib_duplicates`, `lib_classify`, `lib_ingest`, `lib_search`, `lib_status`, `lib_init` - Library management
- `lib_pdf_search` - Full-text PDF/EPUB search
- `lib_sync` - Sync library to MinIO (via CLI: `lib-organizer sync`)
- `secrets_scan` - Find sensitive files

## Testing MCP Server

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | ./target/release/vault-tree-mcp
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | ./target/release/vault-tree-mcp
```

## Syncing to lib.droo.foo

The `lib-organizer sync` command uploads organized documents to MinIO and generates an import manifest for droodotfoo:

```bash
# Dry run
lib-organizer sync --library ~/lib --dry-run

# Full sync
lib-organizer sync --library ~/lib --manifest import.json

# Import to droodotfoo
curl -X POST http://lib.droo.foo/api/import -H "Content-Type: application/json" -d @import.json
```

Credentials from ansible vault (`xochimilco/inventory/group_vars/all/secrets.yml`):
- `vault_minio_access_key` / `vault_minio_secret_key` for S3 operations

## Code Style

- Use `thiserror` for library errors, `anyhow` for applications
- Implement `Display` for user-facing types (not Debug format in output)
- Progress bars: `:: {spinner} {msg} ━{bar}━ {pos}/{len} | ETA {eta}`
- CLI help shows defaults: `[default: current dir]`
