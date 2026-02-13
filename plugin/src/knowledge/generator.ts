import type { KnowledgeEntry } from "./types";

export interface NoteTemplate {
  title: string;
  content: string;
  frontmatter: Record<string, unknown>;
  suggestedPath?: string;
}

export interface GeneratorOptions {
  includeUrl?: boolean;
  includeMetadata?: boolean;
  templateStyle?: "minimal" | "standard" | "detailed";
  folderMapping?: Record<string, string>;
}

const DEFAULT_FOLDER_MAPPING: Record<string, string> = {
  wikipedia: "References",
  wikidata: "References",
  dbpedia: "References",
  wikiart: "Art",
  openlibrary: "Books",
  musicbrainz: "Music",
  arxiv: "Papers",
  shodan: "Security",
  github: "Code",
  sourceforge: "Code",
  defillama: "Crypto",
};

const SOURCE_NAMES: Record<string, string> = {
  wikipedia: "Wikipedia",
  wikidata: "Wikidata",
  dbpedia: "DBpedia",
  wikiart: "WikiArt",
  openlibrary: "OpenLibrary",
  musicbrainz: "MusicBrainz",
  arxiv: "arXiv",
  shodan: "Shodan",
  github: "GitHub",
  sourceforge: "SourceForge",
  defillama: "DefiLlama",
};

const capitalize = (str: string): string =>
  str.charAt(0).toUpperCase() + str.slice(1);

const getSourceName = (source: string): string =>
  SOURCE_NAMES[source] ?? capitalize(source);

const sanitizeFilename = (name: string): string =>
  name.replace(/[<>:"/\\|?*]/g, "").replace(/\s+/g, " ").trim().slice(0, 100);

const formatDuration = (ms: number): string => {
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
};

// Metadata field definitions per source
type MetaField = {
  key: string;
  label: string;
  format?: (val: unknown) => string;
};

const joinArray = (val: unknown, limit?: number): string => {
  const arr = val as string[];
  return (limit ? arr.slice(0, limit) : arr).join(", ");
};

const METADATA_FIELDS: Record<string, MetaField[]> = {
  openlibrary: [
    { key: "authors", label: "Authors", format: joinArray },
    { key: "year", label: "Published" },
    { key: "isbn", label: "ISBN" },
    { key: "subjects", label: "Subjects", format: joinArray },
  ],
  musicbrainz: [
    { key: "artistType", label: "Type" },
    { key: "country", label: "Country" },
    { key: "beginDate", label: "Active from" },
    { key: "tags", label: "Genres", format: (v) => joinArray(v, 5) },
    { key: "artists", label: "Artists", format: joinArray },
    { key: "date", label: "Released" },
    { key: "duration", label: "Duration", format: (v) => formatDuration(v as number) },
  ],
  wikiart: [
    { key: "birthDay", label: "Born" },
    { key: "deathDay", label: "Died" },
    { key: "artist", label: "Artist" },
    { key: "year", label: "Year" },
  ],
  wikidata: [
    { key: "qid", label: "Wikidata ID" },
  ],
  dbpedia: [
    { key: "types", label: "Types", format: joinArray },
    { key: "categories", label: "Categories", format: (v) => joinArray(v, 5) },
  ],
  arxiv: [
    { key: "authors", label: "Authors", format: joinArray },
    { key: "published", label: "Published" },
    { key: "arxivId", label: "arXiv ID" },
    { key: "categories", label: "Categories", format: joinArray },
    { key: "doi", label: "DOI" },
    { key: "pdfLink", label: "PDF", format: (v) => `[Download](${v})` },
  ],
  shodan: [
    { key: "ip", label: "IP" },
    { key: "hostnames", label: "Hostnames", format: joinArray },
    { key: "org", label: "Organization" },
    { key: "country", label: "Country" },
    { key: "ports", label: "Open Ports", format: (v) => (v as number[]).join(", ") },
    { key: "os", label: "OS" },
    { key: "vulns", label: "Vulnerabilities", format: joinArray },
  ],
  github: [
    { key: "owner", label: "Owner" },
    { key: "language", label: "Language" },
    { key: "stars", label: "Stars" },
    { key: "forks", label: "Forks" },
    { key: "license", label: "License" },
    { key: "topics", label: "Topics", format: joinArray },
    { key: "login", label: "Username" },
    { key: "publicRepos", label: "Public Repos" },
    { key: "followers", label: "Followers" },
    { key: "company", label: "Company" },
    { key: "location", label: "Location" },
  ],
  sourceforge: [
    { key: "languages", label: "Languages", format: joinArray },
    { key: "licenses", label: "License", format: joinArray },
    { key: "platforms", label: "Platforms", format: joinArray },
    { key: "downloads", label: "Downloads" },
    { key: "homepage", label: "Homepage" },
  ],
  defillama: [
    { key: "tvlFormatted", label: "TVL" },
    { key: "category", label: "Category" },
    { key: "chains", label: "Chains", format: joinArray },
    { key: "symbol", label: "Token" },
    {
      key: "change1d",
      label: "24h Change",
      format: (v) => `${(v as number) >= 0 ? "+" : ""}${(v as number).toFixed(2)}%`,
    },
    { key: "website", label: "Website" },
  ],
};

const formatMetadataField = (
  meta: Record<string, unknown>,
  field: MetaField
): string | null => {
  const value = meta[field.key];
  if (value === undefined || value === null) return null;

  const formatted = field.format ? field.format(value) : String(value);
  return `- **${field.label}**: ${formatted}`;
};

const formatMetadataSection = (entry: KnowledgeEntry): string | null => {
  if (!entry.metadata) return null;

  const fields = METADATA_FIELDS[entry.source];

  const lines = fields
    ? fields
        .map((field) => formatMetadataField(entry.metadata!, field))
        .filter((line): line is string => line !== null)
    : Object.entries(entry.metadata)
        .filter(([, value]) => value && typeof value !== "object")
        .map(([key, value]) => `- **${capitalize(key)}**: ${value}`);

  return lines.length > 0 ? lines.join("\n") : null;
};

const buildMinimalContent = (entry: KnowledgeEntry, includeUrl: boolean): string =>
  [
    entry.summary,
    ...(includeUrl && entry.url ? ["", `[Source](${entry.url})`] : []),
  ].join("\n");

const buildStandardContent = (
  entry: KnowledgeEntry,
  style: "standard" | "detailed",
  includeUrl: boolean
): string => {
  const sections = [
    `# ${entry.title}`,
    "",
    entry.summary,
  ];

  if (includeUrl && entry.url) {
    sections.push(
      "",
      "## References",
      "",
      `- [${getSourceName(entry.source)}](${entry.url})`
    );
  }

  if (style === "detailed" && entry.metadata) {
    const details = formatMetadataSection(entry);
    if (details) {
      sections.push("", "## Details", "", details);
    }
  }

  sections.push("", "## Notes", "", "<!-- Add your notes here -->");

  return sections.join("\n");
};

const buildContent = (
  entry: KnowledgeEntry,
  style: "minimal" | "standard" | "detailed",
  includeUrl: boolean
): string =>
  style === "minimal"
    ? buildMinimalContent(entry, includeUrl)
    : buildStandardContent(entry, style, includeUrl);

const formatFrontmatterValue = (key: string, value: unknown): string[] => {
  if (value === undefined || value === null) return [];

  if (Array.isArray(value)) {
    return [
      `${key}:`,
      ...value.map((item) => `  - ${JSON.stringify(item)}`),
    ];
  }

  if (typeof value === "object") {
    return [`${key}: ${JSON.stringify(value)}`];
  }

  if (typeof value === "string" && (value.includes(":") || value.includes("#"))) {
    return [`${key}: "${value}"`];
  }

  return [`${key}: ${value}`];
};

const formatFrontmatter = (data: Record<string, unknown>): string =>
  Object.entries(data)
    .flatMap(([key, value]) => formatFrontmatterValue(key, value))
    .join("\n") + "\n";

const flattenMetadata = (metadata: Record<string, unknown>): Record<string, unknown> =>
  Object.fromEntries(
    Object.entries(metadata).filter(
      ([, value]) => value !== undefined && value !== null && (typeof value !== "object" || Array.isArray(value))
    )
  );

export const generateNoteFromEntry = (
  entry: KnowledgeEntry,
  options?: GeneratorOptions
): NoteTemplate => {
  const style = options?.templateStyle ?? "standard";
  const includeUrl = options?.includeUrl !== false;
  const includeMetadata = options?.includeMetadata !== false;

  const baseFrontmatter: Record<string, unknown> = {
    title: entry.title,
    source: entry.source,
    created: new Date().toISOString().split("T")[0],
    ...(includeUrl && entry.url ? { url: entry.url } : {}),
    ...(includeMetadata && entry.metadata ? flattenMetadata(entry.metadata) : {}),
  };

  const folderMapping = options?.folderMapping ?? DEFAULT_FOLDER_MAPPING;
  const folder = folderMapping[entry.source] ?? "References";
  const suggestedPath = `${folder}/${sanitizeFilename(entry.title)}.md`;

  return {
    title: entry.title,
    content: buildContent(entry, style, includeUrl),
    frontmatter: baseFrontmatter,
    suggestedPath,
  };
};

export const generateNotesFromEntries = (
  entries: KnowledgeEntry[],
  options?: GeneratorOptions
): NoteTemplate[] => entries.map((entry) => generateNoteFromEntry(entry, options));

export const formatNoteContent = (template: NoteTemplate): string =>
  `---\n${formatFrontmatter(template.frontmatter)}---\n\n${template.content}`;
