import { App, PluginSettingTab, Setting } from "obsidian";
import type VaultTreePlugin from "./main";

export interface VaultTreeSettings {
  // MCP Server
  enableMcpServer: boolean;
  wsPort: number;
  httpPort: number;
  // Tree
  defaultDepth: number;
  excludePatterns: string[];
  // Publishing
  apiToken: string;
  apiUrl: string;
  defaultAuthor: string;
  // Organization
  inboxFolder: string;
  excludeFolders: string[];
  minConfidence: number;
  autoGenerateFrontmatter: boolean;
}

export const DEFAULT_SETTINGS: VaultTreeSettings = {
  // MCP Server
  enableMcpServer: true,
  wsPort: 22365,
  httpPort: 22366,
  // Tree
  defaultDepth: 0,
  excludePatterns: [],
  // Publishing
  apiToken: "",
  apiUrl: "https://droo.foo/api/posts",
  defaultAuthor: "DROO AMOR",
  // Organization
  inboxFolder: "999 Review",
  excludeFolders: [".obsidian", ".git", "node_modules", "templates"],
  minConfidence: 0.5,
  autoGenerateFrontmatter: true,
};

export class VaultTreeSettingTab extends PluginSettingTab {
  plugin: VaultTreePlugin;

  constructor(app: App, plugin: VaultTreePlugin) {
    super(app, plugin);
    this.plugin = plugin;
  }

  display(): void {
    const { containerEl } = this;

    containerEl.empty();

    // MCP Server Settings
    containerEl.createEl("h2", { text: "MCP Server" });

    new Setting(containerEl)
      .setName("Enable MCP Server")
      .setDesc("Start MCP server for Claude Code integration on plugin load")
      .addToggle((toggle) =>
        toggle
          .setValue(this.plugin.settings.enableMcpServer)
          .onChange(async (value) => {
            this.plugin.settings.enableMcpServer = value;
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName("WebSocket Port")
      .setDesc("Port for WebSocket MCP server (requires restart)")
      .addText((text) =>
        text
          .setPlaceholder("22365")
          .setValue(String(this.plugin.settings.wsPort))
          .onChange(async (value) => {
            const port = parseInt(value, 10);
            if (!isNaN(port) && port > 0 && port < 65536) {
              this.plugin.settings.wsPort = port;
              await this.plugin.saveSettings();
            }
          })
      );

    new Setting(containerEl)
      .setName("HTTP Port")
      .setDesc("Port for HTTP/SSE MCP server (requires restart)")
      .addText((text) =>
        text
          .setPlaceholder("22366")
          .setValue(String(this.plugin.settings.httpPort))
          .onChange(async (value) => {
            const port = parseInt(value, 10);
            if (!isNaN(port) && port > 0 && port < 65536) {
              this.plugin.settings.httpPort = port;
              await this.plugin.saveSettings();
            }
          })
      );

    // Tree Settings
    containerEl.createEl("h2", { text: "Tree Generation" });

    new Setting(containerEl)
      .setName("Default Tree Depth")
      .setDesc("Maximum depth for tree generation (0 = unlimited)")
      .addText((text) =>
        text
          .setPlaceholder("0")
          .setValue(String(this.plugin.settings.defaultDepth))
          .onChange(async (value) => {
            const depth = parseInt(value, 10);
            if (!isNaN(depth) && depth >= 0) {
              this.plugin.settings.defaultDepth = depth;
              await this.plugin.saveSettings();
            }
          })
      );

    // Publishing Settings
    containerEl.createEl("h2", { text: "Publishing (droo.foo)" });

    new Setting(containerEl)
      .setName("API Token")
      .setDesc("Authentication token for publishing to droo.foo")
      .addText((text) =>
        text
          .setPlaceholder("Enter your API token")
          .setValue(this.plugin.settings.apiToken)
          .onChange(async (value) => {
            this.plugin.settings.apiToken = value;
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName("API URL")
      .setDesc("API endpoint for publishing (default: https://droo.foo/api/posts)")
      .addText((text) =>
        text
          .setPlaceholder("https://droo.foo/api/posts")
          .setValue(this.plugin.settings.apiUrl)
          .onChange(async (value) => {
            this.plugin.settings.apiUrl = value || DEFAULT_SETTINGS.apiUrl;
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName("Default Author")
      .setDesc("Default author name for new posts")
      .addText((text) =>
        text
          .setPlaceholder("DROO AMOR")
          .setValue(this.plugin.settings.defaultAuthor)
          .onChange(async (value) => {
            this.plugin.settings.defaultAuthor = value || DEFAULT_SETTINGS.defaultAuthor;
            await this.plugin.saveSettings();
          })
      );

    // Organization Settings
    containerEl.createEl("h2", { text: "Organization" });

    new Setting(containerEl)
      .setName("Inbox Folder")
      .setDesc("Folder to triage for note organization")
      .addText((text) =>
        text
          .setPlaceholder("999 Review")
          .setValue(this.plugin.settings.inboxFolder)
          .onChange(async (value) => {
            this.plugin.settings.inboxFolder = value || DEFAULT_SETTINGS.inboxFolder;
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName("Exclude Folders")
      .setDesc("Folders to exclude from organization suggestions (comma-separated)")
      .addText((text) =>
        text
          .setPlaceholder(".obsidian, .git, node_modules, templates")
          .setValue(this.plugin.settings.excludeFolders.join(", "))
          .onChange(async (value) => {
            this.plugin.settings.excludeFolders = value
              .split(",")
              .map((f) => f.trim())
              .filter((f) => f.length > 0);
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName("Minimum Confidence")
      .setDesc("Minimum confidence threshold for placement suggestions (0.0 - 1.0)")
      .addText((text) =>
        text
          .setPlaceholder("0.5")
          .setValue(String(this.plugin.settings.minConfidence))
          .onChange(async (value) => {
            const conf = parseFloat(value);
            if (!isNaN(conf) && conf >= 0 && conf <= 1) {
              this.plugin.settings.minConfidence = conf;
              await this.plugin.saveSettings();
            }
          })
      );

    new Setting(containerEl)
      .setName("Auto-generate Frontmatter")
      .setDesc("Automatically add frontmatter to markdown files without it during ingest")
      .addToggle((toggle) =>
        toggle
          .setValue(this.plugin.settings.autoGenerateFrontmatter)
          .onChange(async (value) => {
            this.plugin.settings.autoGenerateFrontmatter = value;
            await this.plugin.saveSettings();
          })
      );
  }
}
