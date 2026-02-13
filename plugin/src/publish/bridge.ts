import { requestUrl, RequestUrlResponse } from "obsidian";
import type { PostMetadata, PublishResult } from "./types";
import { stripFrontmatter } from "../utils/frontmatter";

const DEFAULT_API_URL = "https://droo.foo/api/posts";

export interface PublishOptions {
  apiUrl?: string;
  apiToken: string;
  dryRun?: boolean;
}

export async function publishPost(
  content: string,
  metadata: PostMetadata,
  options: PublishOptions
): Promise<PublishResult> {
  const apiUrl = options.apiUrl || DEFAULT_API_URL;

  if (options.dryRun) {
    console.log("Dry run - would publish:", { metadata, contentLength: content.length });
    return {
      success: true,
      url: `https://droo.foo/writing/${metadata.slug}`,
    };
  }

  try {
    const response = await requestUrl({
      url: apiUrl,
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${options.apiToken}`,
      },
      body: JSON.stringify({
        content: stripFrontmatter(content),
        metadata,
      }),
      throw: false,
    });

    return handleResponse(response, metadata.slug);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown error";
    return {
      success: false,
      error: `Network error: ${message}`,
    };
  }
}

export async function updatePost(
  slug: string,
  content: string,
  metadata: PostMetadata,
  options: PublishOptions
): Promise<PublishResult> {
  const apiUrl = options.apiUrl || DEFAULT_API_URL;

  if (options.dryRun) {
    console.log("Dry run - would update:", { slug, metadata });
    return {
      success: true,
      url: `https://droo.foo/writing/${slug}`,
    };
  }

  try {
    const response = await requestUrl({
      url: `${apiUrl}/${slug}`,
      method: "PUT",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${options.apiToken}`,
      },
      body: JSON.stringify({
        content: stripFrontmatter(content),
        metadata,
      }),
      throw: false,
    });

    return handleResponse(response, slug);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown error";
    return {
      success: false,
      error: `Network error: ${message}`,
    };
  }
}

export async function checkPostExists(
  slug: string,
  options: PublishOptions
): Promise<boolean> {
  const apiUrl = options.apiUrl || DEFAULT_API_URL;

  try {
    const response = await requestUrl({
      url: `${apiUrl}/${slug}`,
      method: "HEAD",
      headers: {
        Authorization: `Bearer ${options.apiToken}`,
      },
      throw: false,
    });

    return response.status === 200;
  } catch {
    return false;
  }
}

function handleResponse(response: RequestUrlResponse, slug: string): PublishResult {
  switch (response.status) {
    case 200:
    case 201:
      return {
        success: true,
        url: `https://droo.foo/writing/${slug}`,
      };

    case 401:
      return {
        success: false,
        error: "Unauthorized: Invalid or missing API token",
      };

    case 403:
      return {
        success: false,
        error: "Forbidden: You do not have permission to publish",
      };

    case 404:
      return {
        success: false,
        error: "Not found: The post does not exist",
      };

    case 409:
      return {
        success: false,
        error: "Conflict: A post with this slug already exists",
      };

    case 422:
      const errorData = response.json as { errors?: Record<string, string[]> };
      const errors = errorData?.errors
        ? Object.entries(errorData.errors)
            .map(([field, msgs]) => `${field}: ${msgs.join(", ")}`)
            .join("; ")
        : "Validation failed";
      return {
        success: false,
        error: `Validation error: ${errors}`,
      };

    case 429:
      return {
        success: false,
        error: "Rate limited: Too many requests, please try again later",
      };

    case 500:
    case 502:
    case 503:
      return {
        success: false,
        error: "Server error: Please try again later",
      };

    default:
      return {
        success: false,
        error: `Unexpected response: ${response.status}`,
      };
  }
}

