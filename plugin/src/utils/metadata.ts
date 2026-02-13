import { extractFrontmatterBlock, extractMultiLineTags } from "./frontmatter";

/**
 * Extract tags from markdown content.
 * Handles frontmatter tags (both inline and multi-line) and inline #tags.
 */
export function extractTags(content: string): string[] {
  const tags: string[] = [];

  // Frontmatter tags
  const yaml = extractFrontmatterBlock(content);
  if (yaml) {
    // Inline array format: tags: [tag1, tag2]
    const tagMatch = yaml.match(/tags:\s*\[([^\]]+)\]/);
    if (tagMatch) {
      tags.push(
        ...tagMatch[1]
          .split(",")
          .map((t) => t.trim().replace(/^["']|["']$/g, ""))
      );
    }

    // Multi-line format
    const multiLineTags = extractMultiLineTags(yaml);
    if (multiLineTags) {
      tags.push(...multiLineTags);
    }
  }

  // Inline tags (#tag)
  const inlineTags = content.match(/#[a-zA-Z][a-zA-Z0-9_-]*/g) || [];
  tags.push(...inlineTags.map((t) => t.slice(1)));

  return [...new Set(tags)];
}

const STOP_WORDS = new Set([
  "the", "and", "for", "are", "but", "not", "you", "all", "can", "had",
  "her", "was", "one", "our", "out", "has", "have", "been", "were", "they",
  "this", "that", "with", "from", "your", "will", "more", "when", "which",
  "their", "what", "there", "about", "would", "make", "like", "just", "over",
  "such", "into", "than", "them", "some", "could", "other", "then", "these",
]);

/**
 * Extract keywords from markdown content.
 * Removes frontmatter, code blocks, links, and stop words.
 * Returns a map of word -> count.
 */
export function extractKeywords(content: string): Record<string, number> {
  // Remove frontmatter
  let text = content;
  if (text.startsWith("---")) {
    const endIndex = text.indexOf("\n---", 3);
    if (endIndex !== -1) {
      text = text.slice(endIndex + 4);
    }
  }

  // Remove code blocks
  text = text.replace(/```[\s\S]*?```/g, "");
  text = text.replace(/`[^`]+`/g, "");

  // Remove links
  text = text.replace(/\[\[([^\]]+)\]\]/g, "$1");
  text = text.replace(/\[([^\]]+)\]\([^)]+\)/g, "$1");

  // Extract words
  const words = text
    .toLowerCase()
    .replace(/[^a-z0-9\s]/g, " ")
    .split(/\s+/)
    .filter((w) => w.length > 3 && !STOP_WORDS.has(w));

  const counts: Record<string, number> = {};
  for (const word of words) {
    counts[word] = (counts[word] || 0) + 1;
  }

  return counts;
}
