import { requestUrl } from "obsidian";
import type { AIProvider, SuggestLinksResult, VaultContext, LinkSuggestion } from "../../types";

const DEFAULT_OLLAMA_URL = "http://localhost:11434";
const DEFAULT_MODEL = "llama3.2";

export interface OllamaConfig {
  baseUrl?: string;
  model?: string;
}

interface OllamaGenerateResponse {
  response?: string;
  done?: boolean;
  error?: string;
}

interface SuggestionsResponse {
  suggestions?: Array<{
    target: string;
    confidence: number;
    reason: string;
    text?: string;
  }>;
}

export class OllamaProvider implements AIProvider {
  readonly name = "ollama";
  private baseUrl: string;
  private model: string;

  constructor(config?: OllamaConfig) {
    this.baseUrl = config?.baseUrl ?? DEFAULT_OLLAMA_URL;
    this.model = config?.model ?? DEFAULT_MODEL;
  }

  configure(config: OllamaConfig): void {
    if (config.baseUrl) this.baseUrl = config.baseUrl;
    if (config.model) this.model = config.model;
  }

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${this.baseUrl}/api/tags`,
        method: "GET",
        throw: false,
      });
      return response.status === 200;
    } catch {
      return false;
    }
  }

  async suggestLinks(
    noteContent: string,
    notePath: string,
    vaultContext: VaultContext
  ): Promise<SuggestLinksResult> {
    try {
      const prompt = this.buildPrompt(noteContent, notePath, vaultContext);

      const response = await requestUrl({
        url: `${this.baseUrl}/api/generate`,
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          model: this.model,
          prompt,
          stream: false,
          format: "json",
        }),
        throw: false,
      });

      if (response.status !== 200) {
        return {
          success: false,
          provider: this.name,
          suggestions: [],
          error: `Ollama request failed: ${response.status}`,
        };
      }

      const data = response.json as OllamaGenerateResponse;

      if (data.error) {
        return {
          success: false,
          provider: this.name,
          suggestions: [],
          error: `Ollama error: ${data.error}`,
        };
      }

      const suggestions = this.parseResponse(data.response ?? "");

      return {
        success: true,
        provider: this.name,
        suggestions,
      };
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      return {
        success: false,
        provider: this.name,
        suggestions: [],
        error: `Ollama request failed: ${message}`,
      };
    }
  }

  private buildPrompt(noteContent: string, notePath: string, context: VaultContext): string {
    const availableNotes = context.noteTitles
      .filter((title) => !notePath.includes(title))
      .slice(0, 100)
      .join("\n- ");

    const availableTags = context.tags.slice(0, 50).join(", ");

    return `You are an assistant that suggests internal links for an Obsidian note.

Given the following note content and available notes in the vault, suggest relevant internal links that would enrich the note.

Note path: ${notePath}

Note content:
${noteContent.slice(0, 4000)}

Available notes in vault:
- ${availableNotes}

Available tags: ${availableTags}

Respond with a JSON object containing a "suggestions" array. Each suggestion should have:
- "target": the exact title of an existing note to link to
- "confidence": a number from 0 to 1 indicating how relevant the link is
- "reason": a brief explanation of why this link is relevant
- "text": (optional) suggested anchor text for the link

Only suggest links to notes that exist in the available notes list.
Return at most 10 suggestions, sorted by confidence descending.

JSON response:`;
  }

  private parseResponse(response: string): LinkSuggestion[] {
    try {
      const cleaned = response.trim();
      const parsed = JSON.parse(cleaned) as SuggestionsResponse;

      if (!parsed.suggestions || !Array.isArray(parsed.suggestions)) {
        console.warn("[ollama] Response missing suggestions array:", response.slice(0, 200));
        return [];
      }

      return parsed.suggestions
        .filter((s) => s.target && typeof s.confidence === "number")
        .map((s) => ({
          targetNote: s.target,
          confidence: Math.max(0, Math.min(1, s.confidence)),
          reason: s.reason ?? "",
          suggestedText: s.text,
        }))
        .sort((a, b) => b.confidence - a.confidence);
    } catch (error) {
      console.warn("[ollama] Failed to parse response:", error, response.slice(0, 200));
      return [];
    }
  }
}
