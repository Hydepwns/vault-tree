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

This is a Rust workspace with four crates:

### vault-tree-core
Shared library for Obsidian vault parsing:
- `tree.rs` - Generate annotated vault trees with tags, dates, link counts
- `frontmatter.rs` - YAML frontmatter extraction
- `links.rs` - Wikilink parsing and backlink indexing
- `search.rs` - Regex search across markdown files
- `fingerprint.rs` - Content hashing

### vault-tree-mcp
Standalone MCP server (JSON-RPC over stdio):
- `server.rs` - MCP protocol handler (initialize, tools/list, tools/call)
- `tools.rs` - Tool definitions and implementations
- `transport.rs` - JSON-RPC types

### lib-organizer
Document library organizer with CLI:
- `scanner.rs` - Scan directories for PDF/EPUB/DJVU/MOBI files
- `classifier.rs` - Topic classification by filename keywords
- `organizer.rs` - Ingest files into organized library with git tracking
- `search/` - Tantivy-based full-text PDF/EPUB search
- `secrets.rs` - Detect sensitive files (private keys, credentials)
- `cli.rs` - Binary with subcommands: init, scan, ingest, search, etc.

### vault-tree-wasm
WASM bindings for browser/Obsidian plugin use.

## MCP Tools

The MCP server exposes these tools:
- `vault_tree`, `vault_search` - Obsidian vault operations
- `lib_scan`, `lib_duplicates`, `lib_classify`, `lib_ingest`, `lib_search`, `lib_status`, `lib_init` - Library management
- `lib_pdf_search` - Full-text search in PDF/EPUB content
- `secrets_scan` - Find sensitive files

## Testing MCP Server

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | ./target/release/vault-tree-mcp
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | ./target/release/vault-tree-mcp
```

## Code Style

- Use `thiserror` for library errors, `anyhow` for applications
- Implement `Display` for user-facing types (not Debug format in output)
- Progress bars use minimal style: `:: {spinner} {msg} ━{bar}━ {pos}/{len} | ETA {eta}`
- CLI help text should show defaults: `[default: current dir]`
- Final status messages end with periods
