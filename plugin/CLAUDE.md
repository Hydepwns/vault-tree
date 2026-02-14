# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
npm install      # install dependencies
npm run build    # production build
npm run dev      # watch mode for development
```

Output: `main.js` in plugin root (loaded by Obsidian)

## Architecture

Obsidian plugin exposing vault operations via MCP servers. Claude Code connects via WebSocket (port 22365) or HTTP (port 22366).

### Core Flow

```
main.ts (Plugin)
    |
    +-- McpServer (mcp/server.ts)
    |       |
    |       +-- WebSocket server (port 22365) - CLI clients
    |       +-- HTTP/SSE server (port 22366) - Desktop clients
    |       |
    |       +-- tools/ (modular tool handlers)
    |               +-- vault.ts, organize.ts, knowledge.ts
    |               +-- links.ts, publish.ts
    |
    +-- knowledge/ (15 providers + LRU cache)
    +-- Settings UI (settings.ts)
```

### Knowledge Providers

15 providers in `knowledge/providers/`:
- General: wikipedia, dbpedia, wikidata
- Code: github, sourceforge, npm, crates.io, stackoverflow
- Social: reddit
- Reference: openlibrary, arxiv, musicbrainz, wikiart
- Specialized: defillama, shodan

Registry with LRU cache (100 items, 15min TTL). Configure via `configureShodan(apiKey)` and `configureGitHub(token)`.

### Adding Providers

**Knowledge Provider:**
1. Implement `KnowledgeProvider` interface in `knowledge/providers/`
2. Register in `knowledge/registry.ts` (add to createDefaultProviders, PROVIDER_ORDER, type union)
3. Add enum value in `mcp/tools/knowledge.ts` schema

**AI Provider:**
1. Implement `AIProvider` interface in `knowledge/providers/ai/`
2. Add config type and register in settings

### Patterns

- Functional style: prefer `filter/map/reduce` over imperative loops
- Discriminated unions for result types
- Error handling: log with `[module]` prefix, never swallow silently

## MCP Tools

| Category | Tools |
|----------|-------|
| Vault | `vault_tree`, `vault_search`, `find_duplicates` |
| Organize | `organize_triage`, `organize_ingest` |
| Knowledge | `knowledge_lookup`, `create_note` |
| Links | `suggest_links`, `apply_links`, `batch_suggest_links` |
| Publish | `publish_post`, `validate_post` |
