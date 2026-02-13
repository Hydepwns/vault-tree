import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const DEFILLAMA_API = "https://api.llama.fi";

interface DefiProtocol {
  id: string;
  name: string;
  slug: string;
  symbol?: string;
  url?: string;
  description?: string;
  chain?: string;
  chains?: string[];
  tvl?: number;
  chainTvls?: Record<string, number>;
  change_1h?: number;
  change_1d?: number;
  change_7d?: number;
  category?: string;
  logo?: string;
  twitter?: string;
  gecko_id?: string;
  mcap?: number;
}

interface DefiChain {
  name: string;
  tvl: number;
  chainId?: number;
  gecko_id?: string;
  tokenSymbol?: string;
}

export class DefiLlamaProvider implements KnowledgeProvider {
  readonly name = "defillama";

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${DEFILLAMA_API}/protocols`,
        method: "GET",
        throw: false,
      });
      return response.status === 200;
    } catch {
      return false;
    }
  }

  async lookup(query: string, options?: LookupOptions): Promise<LookupResult> {
    const maxResults = options?.maxResults ?? 5;

    try {
      // Check if query is a chain name
      const chainEntry = await this.lookupChain(query);
      if (chainEntry) {
        return {
          success: true,
          provider: this.name,
          entries: [chainEntry],
        };
      }

      // Search protocols
      const entries = await this.searchProtocols(query, maxResults);

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
        error: `DefiLlama lookup failed: ${message}`,
      };
    }
  }

  private async searchProtocols(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${DEFILLAMA_API}/protocols`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`Failed to fetch protocols: ${response.status}`);
    }

    const protocols = response.json as DefiProtocol[];
    const queryLower = query.toLowerCase();

    // Filter and score matches
    const matches = protocols
      .filter((p) => {
        const nameMatch = p.name.toLowerCase().includes(queryLower);
        const symbolMatch = p.symbol?.toLowerCase().includes(queryLower);
        const categoryMatch = p.category?.toLowerCase().includes(queryLower);
        return nameMatch || symbolMatch || categoryMatch;
      })
      .sort((a, b) => {
        // Exact name match first
        const aExact = a.name.toLowerCase() === queryLower ? 1 : 0;
        const bExact = b.name.toLowerCase() === queryLower ? 1 : 0;
        if (aExact !== bExact) return bExact - aExact;

        // Then by TVL
        return (b.tvl ?? 0) - (a.tvl ?? 0);
      })
      .slice(0, limit);

    return matches.map((protocol) => this.protocolToEntry(protocol));
  }

  private async lookupChain(name: string): Promise<KnowledgeEntry | null> {
    const response = await requestUrl({
      url: `${DEFILLAMA_API}/v2/chains`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      return null;
    }

    const chains = response.json as DefiChain[];
    const nameLower = name.toLowerCase();

    const chain = chains.find(
      (c) => c.name.toLowerCase() === nameLower
    );

    if (!chain) {
      return null;
    }

    const tvlFormatted = this.formatTvl(chain.tvl);

    return {
      title: chain.name,
      summary: `Blockchain with ${tvlFormatted} TVL`,
      url: `https://defillama.com/chain/${chain.name}`,
      source: "defillama",
      metadata: {
        type: "chain",
        tvl: chain.tvl,
        tvlFormatted,
        chainId: chain.chainId,
        geckoId: chain.gecko_id,
        tokenSymbol: chain.tokenSymbol,
      },
    };
  }

  private protocolToEntry(protocol: DefiProtocol): KnowledgeEntry {
    const lines: string[] = [];

    if (protocol.description) {
      lines.push(protocol.description.slice(0, 200));
    }

    const tvl = this.formatTvl(protocol.tvl);
    lines.push(`TVL: ${tvl}`);

    if (protocol.category) {
      lines.push(`Category: ${protocol.category}`);
    }

    if (protocol.chains && protocol.chains.length > 0) {
      lines.push(`Chains: ${protocol.chains.slice(0, 5).join(", ")}`);
    }

    if (protocol.change_1d !== undefined) {
      const changeSign = protocol.change_1d >= 0 ? "+" : "";
      lines.push(`24h Change: ${changeSign}${protocol.change_1d.toFixed(2)}%`);
    }

    return {
      title: protocol.name,
      summary: lines.join("\n"),
      url: `https://defillama.com/protocol/${protocol.slug}`,
      source: "defillama",
      metadata: {
        type: "protocol",
        id: protocol.id,
        slug: protocol.slug,
        symbol: protocol.symbol,
        category: protocol.category,
        tvl: protocol.tvl,
        tvlFormatted: tvl,
        chains: protocol.chains,
        change1h: protocol.change_1h,
        change1d: protocol.change_1d,
        change7d: protocol.change_7d,
        mcap: protocol.mcap,
        twitter: protocol.twitter,
        geckoId: protocol.gecko_id,
        logo: protocol.logo,
        website: protocol.url,
      },
    };
  }

  private formatTvl(tvl?: number): string {
    if (!tvl) return "N/A";

    if (tvl >= 1e9) {
      return `$${(tvl / 1e9).toFixed(2)}B`;
    } else if (tvl >= 1e6) {
      return `$${(tvl / 1e6).toFixed(2)}M`;
    } else if (tvl >= 1e3) {
      return `$${(tvl / 1e3).toFixed(2)}K`;
    }
    return `$${tvl.toFixed(2)}`;
  }

  async getProtocolTvlHistory(slug: string): Promise<Array<{ date: number; tvl: number }> | null> {
    const response = await requestUrl({
      url: `${DEFILLAMA_API}/protocol/${slug}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      return null;
    }

    const data = response.json as { tvl?: Array<{ date: number; totalLiquidityUSD: number }> };

    if (!data.tvl) {
      return null;
    }

    return data.tvl.map((point) => ({
      date: point.date,
      tvl: point.totalLiquidityUSD,
    }));
  }
}
