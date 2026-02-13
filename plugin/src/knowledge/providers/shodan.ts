import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const SHODAN_API = "https://api.shodan.io";

interface ShodanHostResult {
  ip_str?: string;
  hostnames?: string[];
  org?: string;
  isp?: string;
  asn?: string;
  country_name?: string;
  city?: string;
  os?: string;
  ports?: number[];
  vulns?: string[];
  tags?: string[];
  data?: Array<{
    port: number;
    transport: string;
    product?: string;
    version?: string;
    banner?: string;
  }>;
}

interface ShodanSearchResult {
  matches?: Array<{
    ip_str: string;
    port: number;
    org?: string;
    hostnames?: string[];
    product?: string;
    os?: string;
    country_name?: string;
    asn?: string;
  }>;
  total?: number;
}

interface ShodanDnsResult {
  [domain: string]: {
    type: string;
    value: string;
    subdomain?: string;
  }[];
}

export interface ShodanConfig {
  apiKey?: string;
}

export class ShodanProvider implements KnowledgeProvider {
  readonly name = "shodan";
  private apiKey: string;

  constructor(config?: ShodanConfig) {
    this.apiKey = config?.apiKey ?? "";
  }

  configure(config: ShodanConfig): void {
    if (config.apiKey !== undefined) this.apiKey = config.apiKey;
  }

  async isAvailable(): Promise<boolean> {
    return this.apiKey.length > 0;
  }

  async lookup(query: string, options?: LookupOptions): Promise<LookupResult> {
    const maxResults = options?.maxResults ?? 5;

    if (!this.apiKey) {
      return {
        success: false,
        provider: this.name,
        entries: [],
        error: "Shodan API key not configured",
      };
    }

    try {
      // Check if query is an IP address
      if (this.isIpAddress(query)) {
        const entry = await this.lookupHost(query);
        return {
          success: true,
          provider: this.name,
          entries: entry ? [entry] : [],
        };
      }

      // Check if query is a domain
      if (this.isDomain(query)) {
        const entries = await this.lookupDns(query);
        return {
          success: true,
          provider: this.name,
          entries: entries.slice(0, maxResults),
        };
      }

      // Otherwise, search
      const entries = await this.search(query, maxResults);
      return {
        success: true,
        provider: this.name,
        entries,
      };
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      return {
        success: false,
        provider: this.name,
        entries: [],
        error: `Shodan lookup failed: ${message}`,
      };
    }
  }

  private isIpAddress(query: string): boolean {
    return /^(\d{1,3}\.){3}\d{1,3}$/.test(query);
  }

  private isDomain(query: string): boolean {
    return /^[a-zA-Z0-9][a-zA-Z0-9-]*\.[a-zA-Z]{2,}$/.test(query);
  }

  private async lookupHost(ip: string): Promise<KnowledgeEntry | null> {
    const response = await requestUrl({
      url: `${SHODAN_API}/shodan/host/${ip}?key=${this.apiKey}`,
      method: "GET",
      throw: false,
    });

    if (response.status === 404) {
      return null;
    }

    if (response.status !== 200) {
      throw new Error(`Host lookup failed: ${response.status}`);
    }

    const data = response.json as ShodanHostResult;

    const hostnames = data.hostnames?.join(", ") || "No hostnames";
    const location = [data.city, data.country_name].filter(Boolean).join(", ");
    const ports = data.ports?.slice(0, 10).join(", ") || "None";

    const lines: string[] = [];
    lines.push(`IP: ${data.ip_str}`);
    if (data.org) lines.push(`Organization: ${data.org}`);
    if (location) lines.push(`Location: ${location}`);
    lines.push(`Open Ports: ${ports}`);
    if (data.os) lines.push(`OS: ${data.os}`);
    if (data.vulns && data.vulns.length > 0) {
      lines.push(`Vulnerabilities: ${data.vulns.slice(0, 5).join(", ")}`);
    }

    return {
      title: `${data.ip_str} (${hostnames})`,
      summary: lines.join("\n"),
      url: `https://www.shodan.io/host/${ip}`,
      source: "shodan",
      metadata: {
        ip: data.ip_str,
        hostnames: data.hostnames,
        org: data.org,
        isp: data.isp,
        asn: data.asn,
        country: data.country_name,
        city: data.city,
        os: data.os,
        ports: data.ports,
        vulns: data.vulns,
        tags: data.tags,
      },
    };
  }

  private async lookupDns(domain: string): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${SHODAN_API}/dns/domain/${domain}?key=${this.apiKey}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`DNS lookup failed: ${response.status}`);
    }

    const data = response.json as ShodanDnsResult;
    const entries: KnowledgeEntry[] = [];

    // Return domain info as a single entry
    const records: string[] = [];
    for (const [subdomain, recordList] of Object.entries(data)) {
      if (Array.isArray(recordList)) {
        for (const record of recordList.slice(0, 5)) {
          records.push(`${subdomain}: ${record.type} -> ${record.value}`);
        }
      }
    }

    if (records.length > 0) {
      entries.push({
        title: domain,
        summary: `DNS Records:\n${records.slice(0, 10).join("\n")}`,
        url: `https://www.shodan.io/domain/${domain}`,
        source: "shodan",
        metadata: {
          domain,
          recordCount: records.length,
        },
      });
    }

    return entries;
  }

  private async search(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const params = new URLSearchParams({
      key: this.apiKey,
      query,
    });

    const response = await requestUrl({
      url: `${SHODAN_API}/shodan/host/search?${params}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`Search failed: ${response.status}`);
    }

    const data = response.json as ShodanSearchResult;

    if (!data.matches) {
      return [];
    }

    return data.matches.slice(0, limit).map((match) => {
      const hostnames = match.hostnames?.join(", ") || "No hostnames";
      const lines: string[] = [];
      lines.push(`Port: ${match.port}`);
      if (match.org) lines.push(`Org: ${match.org}`);
      if (match.product) lines.push(`Product: ${match.product}`);
      if (match.country_name) lines.push(`Country: ${match.country_name}`);

      return {
        title: `${match.ip_str}:${match.port} (${hostnames})`,
        summary: lines.join(" | "),
        url: `https://www.shodan.io/host/${match.ip_str}`,
        source: "shodan",
        metadata: {
          ip: match.ip_str,
          port: match.port,
          org: match.org,
          hostnames: match.hostnames,
          product: match.product,
          os: match.os,
          country: match.country_name,
          asn: match.asn,
        },
      };
    });
  }
}
