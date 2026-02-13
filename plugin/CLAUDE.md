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

Obsidian plugin exposing vault operations via MCP (Model Context Protocol) servers. Claude Code connects via WebSocket (port 22365) or HTTP (port 22366).

### Core Flow

```
main.ts (Plugin)
    |
    +-- McpServer (mcp/server.ts)
    |       |
    |       +-- WebSocket server (port 22365) - CLI clients
    |       +-- HTTP/SSE server (port 22366) - Desktop clients
    |       |
    |       +-- tools.ts (tool definitions + handlers)
    |               |
    |               +-- tree/      (vault_tree, vault_search)
    |               +-- organize/  (triage, ingest, duplicates)
    |               +-- knowledge/ (lookup, link suggestions)
    |               +-- publish/   (blog publishing)
    |
    +-- Settings UI (settings.ts)
    +-- Modal UIs (triage-modal, preview)
```

### Key Interfaces

**Knowledge Providers** (`knowledge/types.ts`):
- `KnowledgeProvider` - external data sources (wikipedia, arxiv, github, etc.)
- `AIProvider` - link suggestion providers (ollama, openai)
- Both implement `isAvailable()` and their main lookup/suggest method

**Organization** (`organize/types.ts`):
- `PlacementSuggestion` - folder recommendation with confidence score
- `FolderStats` - aggregated tags/keywords per folder for matching
- `TriageItem` - user decision state for inbox processing

### Adding Providers

**Knowledge Provider:**
1. Implement `KnowledgeProvider` interface in `knowledge/providers/`
2. Register in `knowledge/registry.ts`
3. Add to provider union type and settings dropdown
4. Add enum value in `mcp/tools.ts` schema
5. Update generator folder mapping in `knowledge/generator.ts`

**AI Provider:**
1. Implement `AIProvider` interface in `knowledge/providers/ai/`
2. Add config type and register in settings

### Patterns

- Functional style: prefer `filter/map/reduce` over imperative loops
- Discriminated unions for result types (e.g., `ProcessedFile` in ingest.ts)
- Pure functions for logic, side effects isolated in orchestrating functions
- Error handling: always log with `[module]` prefix, never swallow silently

## MCP Tools

| Category | Tools |
|----------|-------|
| Vault | `vault_tree`, `vault_search`, `find_duplicates` |
| Organize | `organize_triage`, `organize_ingest` |
| Knowledge | `knowledge_lookup`, `create_note` |
| Links | `suggest_links`, `apply_links`, `batch_suggest_links` |
| Publish | `publish_post`, `validate_post` |
