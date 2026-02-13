import type {
  PostMetadata,
  ValidationResult,
  ValidationError,
  ValidationWarning,
  PostStats,
} from "./types";
import { getWasm } from "../wasm/loader";
import {
  stripFrontmatter,
  extractFrontmatterBlock,
  extractMultiLineTags,
} from "../utils/frontmatter";

const DATE_REGEX = /^\d{4}-\d{2}-\d{2}$/;
const SLUG_REGEX = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

export function validateMetadata(metadata: Partial<PostMetadata>): ValidationResult {
  const errors: ValidationError[] = [];
  const warnings: ValidationWarning[] = [];

  // Required fields
  if (!metadata.title || metadata.title.trim() === "") {
    errors.push({ field: "title", message: "Title is required" });
  } else if (metadata.title.length > 100) {
    warnings.push({ field: "title", message: "Title is longer than 100 characters" });
  }

  if (!metadata.date) {
    errors.push({ field: "date", message: "Date is required" });
  } else if (!DATE_REGEX.test(metadata.date)) {
    errors.push({ field: "date", message: "Date must be in YYYY-MM-DD format" });
  }

  if (!metadata.description || metadata.description.trim() === "") {
    errors.push({ field: "description", message: "Description is required" });
  } else if (metadata.description.length > 300) {
    warnings.push({ field: "description", message: "Description is longer than 300 characters" });
  }

  if (!metadata.tags || metadata.tags.length === 0) {
    errors.push({ field: "tags", message: "At least one tag is required" });
  }

  if (!metadata.slug || metadata.slug.trim() === "") {
    errors.push({ field: "slug", message: "Slug is required" });
  } else if (!SLUG_REGEX.test(metadata.slug)) {
    errors.push({
      field: "slug",
      message: "Slug must be lowercase, alphanumeric with hyphens only",
    });
  }

  // Optional field validation
  if (metadata.series_order !== undefined) {
    if (!Number.isInteger(metadata.series_order) || metadata.series_order < 1) {
      errors.push({ field: "series_order", message: "Series order must be a positive integer" });
    }
    if (!metadata.series) {
      warnings.push({ field: "series", message: "Series order set but no series name provided" });
    }
  }

  return {
    valid: errors.length === 0,
    errors,
    warnings,
  };
}

export function extractMetadataFromFrontmatter(content: string): Partial<PostMetadata> | null {
  const wasm = getWasm();
  if (!wasm) {
    return extractMetadataManually(content);
  }

  try {
    const fm = wasm.parse_frontmatter(content);
    return {
      title: fm.title,
      date: fm.date,
      description: fm.description,
      tags: fm.tags || [],
      slug: fm.slug,
    };
  } catch {
    return extractMetadataManually(content);
  }
}

function extractMetadataManually(content: string): Partial<PostMetadata> | null {
  const yamlContent = extractFrontmatterBlock(content);
  if (!yamlContent) {
    return null;
  }

  const metadata: Partial<PostMetadata> = {};

  for (const line of yamlContent.split("\n")) {
    const colonIndex = line.indexOf(":");
    if (colonIndex === -1) continue;

    const key = line.slice(0, colonIndex).trim();
    let value = line.slice(colonIndex + 1).trim();

    // Remove quotes
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }

    switch (key) {
      case "title":
        metadata.title = value;
        break;
      case "date":
        metadata.date = value;
        break;
      case "description":
        metadata.description = value;
        break;
      case "slug":
        metadata.slug = value;
        break;
      case "author":
        metadata.author = value;
        break;
      case "pattern_style":
        metadata.pattern_style = value;
        break;
      case "series":
        metadata.series = value;
        break;
      case "series_order":
        metadata.series_order = parseInt(value, 10);
        break;
      case "reading_time":
        metadata.reading_time = value;
        break;
      case "tags":
        // Handle inline array [tag1, tag2]
        if (value.startsWith("[") && value.endsWith("]")) {
          metadata.tags = value
            .slice(1, -1)
            .split(",")
            .map((t) => t.trim().replace(/^["']|["']$/g, ""));
        }
        break;
    }
  }

  // Handle multi-line tags
  if (!metadata.tags) {
    const multiLineTags = extractMultiLineTags(yamlContent);
    if (multiLineTags) {
      metadata.tags = multiLineTags;
    }
  }

  return metadata;
}

export function calculatePostStats(content: string): PostStats {
  const bodyContent = stripFrontmatter(content);

  // Count words (simple approximation)
  const words = bodyContent
    .replace(/```[\s\S]*?```/g, "") // Remove code blocks
    .replace(/`[^`]+`/g, "") // Remove inline code
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1") // Replace links with text
    .replace(/[#*_~`]/g, "") // Remove markdown symbols
    .split(/\s+/)
    .filter((word) => word.length > 0);

  const wordCount = words.length;
  const characterCount = bodyContent.length;

  // Estimate reading time (200 words per minute)
  const minutes = Math.ceil(wordCount / 200);
  const estimatedReadingTime = minutes === 1 ? "1 min read" : `${minutes} min read`;

  // Count links
  const linkCount = (bodyContent.match(/\[\[|\]\(|https?:\/\//g) || []).length;

  // Count images
  const imageCount = (bodyContent.match(/!\[|\.png|\.jpg|\.jpeg|\.gif|\.webp/gi) || []).length;

  return {
    wordCount,
    characterCount,
    estimatedReadingTime,
    linkCount,
    imageCount,
  };
}

export function generateSlug(title: string): string {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

export function getTodayDate(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}
