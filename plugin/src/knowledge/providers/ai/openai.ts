import { requestUrl } from "obsidian";
import type { AIProvider, SuggestLinksResult, VaultContext, LinkSuggestion } from "../../types";

const DEFAULT_OPENAI_URL = "https://api.openai.com/v1";
const DEFAULT_MODEL = "gpt-4o-mini";

export interface OpenAIConfig {
  apiKey?: string;
  baseUrl?: string;
  model?: string;
}

interface ChatCompletionResponse {
  choices?: Array<{
    message?: {
      content?: string;
    };
  }>;
  error?: {
    message: string;
  };
}

interface SuggestionsResponse {
  suggestions?: Array<{
    target: string;
    confidence: number;
    reason: string;
    text?: string;
  }>;
}

export class OpenAIProvider implements AIProvider {
  readonly name = "openai";
  private apiKey: string;
  private baseUrl: string;
  private model: string;

  constructor(config?: OpenAIConfig) {
    this.apiKey = config?.apiKey ?? "";
    this.baseUrl = config?.baseUrl ?? DEFAULT_OPENAI_URL;
    this.model = config?.model ?? DEFAULT_MODEL;
  }

  configure(config: OpenAIConfig): void {
    if (config.apiKey !== undefined) this.apiKey = config.apiKey;
    if (config.baseUrl) this.baseUrl = config.baseUrl;
    if (config.model) this.model = config.model;
  }

  async isAvailable(): Promise<boolean> {
    return this.apiKey.length > 0;
  }

  async suggestLinks(
    noteContent: string,
    notePath: string,
    vaultContext: VaultContext
  ): Promise<SuggestLinksResult> {
    if (!this.apiKey) {
      return {
        success: false,
        provider: this.name,
        suggestions: [],
        error: "OpenAI API key not configured",
      };
    }

    try {
      const prompt = this.buildPrompt(noteContent, notePath, vaultContext);

      const response = await requestUrl({
        url: `${this.baseUrl}/chat/completions`,
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${this.apiKey}`,
        },
        body: JSON.stringify({
          model: this.model,
          messages: [
            {
              role: "system",
              content:
                "You suggest internal links for Obsidian notes. Respond only with valid JSON.",
            },
            {
              role: "user",
              content: prompt,
            },
          ],
          response_format: { type: "json_object" },
          temperature: 0.3,
        }),
        throw: false,
      });

      if (response.status !== 200) {
        const data = response.json as ChatCompletionResponse;
        const errorMsg = data.error?.message ?? `HTTP ${response.status}`;
        return {
          success: false,
          provider: this.name,
          suggestions: [],
          error: `OpenAI request failed: ${errorMsg}`,
        };
      }

      const data = response.json as ChatCompletionResponse;
      const content = data.choices?.[0]?.message?.content ?? "";
      const suggestions = this.parseResponse(content);

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
        error: `OpenAI request failed: ${message}`,
      };
    }
  }

  private buildPrompt(noteContent: string, notePath: string, context: VaultContext): string {
    const availableNotes = context.noteTitles
      .filter((title) => !notePath.includes(title))
      .slice(0, 100)
      .join("\n- ");

    const availableTags = context.tags.slice(0, 50).join(", ");

    return `Suggest internal links for this Obsidian note.

Note path: ${notePath}

Note content:
${noteContent.slice(0, 4000)}

Available notes in vault:
- ${availableNotes}

Available tags: ${availableTags}

Return a JSON object with a "suggestions" array. Each suggestion:
- "target": exact title of an existing note (from the list above)
- "confidence": 0-1 relevance score
- "reason": brief explanation
- "text": (optional) anchor text

Only suggest notes from the available list. Max 10 suggestions, sorted by confidence.`;
  }

  private parseResponse(response: string): LinkSuggestion[] {
    try {
      const parsed = JSON.parse(response) as SuggestionsResponse;

      if (!parsed.suggestions || !Array.isArray(parsed.suggestions)) {
        console.warn("[openai] Response missing suggestions array:", response.slice(0, 200));
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
      console.warn("[openai] Failed to parse response:", error, response.slice(0, 200));
      return [];
    }
  }
}
