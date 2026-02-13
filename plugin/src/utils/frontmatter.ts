/**
 * Strip frontmatter from markdown content.
 * Returns the content without the YAML frontmatter block.
 */
export function stripFrontmatter(content: string): string {
  const trimmed = content.trim();
  if (!trimmed.startsWith("---")) {
    return content;
  }

  const endIndex = trimmed.indexOf("\n---", 3);
  if (endIndex === -1) {
    return content;
  }

  return trimmed.slice(endIndex + 4).trim();
}

/**
 * Extract the raw YAML frontmatter block from content.
 * Returns null if no frontmatter is found.
 */
export function extractFrontmatterBlock(content: string): string | null {
  const trimmed = content.trim();
  if (!trimmed.startsWith("---")) {
    return null;
  }

  const endIndex = trimmed.indexOf("\n---", 3);
  if (endIndex === -1) {
    return null;
  }

  return trimmed.slice(4, endIndex);
}

/**
 * Parse a simple YAML frontmatter block into key-value pairs.
 * Handles basic YAML: strings, numbers, and inline arrays.
 */
export function parseFrontmatterYaml(yaml: string): Record<string, unknown> {
  const result: Record<string, unknown> = {};

  for (const line of yaml.split("\n")) {
    const colonIndex = line.indexOf(":");
    if (colonIndex === -1) continue;

    const key = line.slice(0, colonIndex).trim();
    let value = line.slice(colonIndex + 1).trim();

    // Skip empty values (likely multi-line)
    if (!value) continue;

    // Remove quotes
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      result[key] = value.slice(1, -1);
    }
    // Handle inline arrays [item1, item2]
    else if (value.startsWith("[") && value.endsWith("]")) {
      result[key] = value
        .slice(1, -1)
        .split(",")
        .map((t) => t.trim().replace(/^["']|["']$/g, ""));
    }
    // Handle numbers
    else if (/^-?\d+(\.\d+)?$/.test(value)) {
      result[key] = parseFloat(value);
    }
    // Handle booleans
    else if (value === "true") {
      result[key] = true;
    } else if (value === "false") {
      result[key] = false;
    }
    // Plain string
    else {
      result[key] = value;
    }
  }

  return result;
}

/**
 * Extract multi-line tags from YAML content.
 * Handles the format:
 * tags:
 *   - tag1
 *   - tag2
 */
export function extractMultiLineTags(yaml: string): string[] | null {
  const match = yaml.match(/tags:\s*\n((?:\s+-\s+.+\n?)+)/);
  if (!match) return null;

  return match[1]
    .split("\n")
    .map((line) => line.replace(/^\s*-\s*/, "").trim())
    .filter((t) => t.length > 0);
}
