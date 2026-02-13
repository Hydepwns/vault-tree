import { App, Editor, Modal, Notice, Plugin, TFile } from "obsidian";
import { initWasm, getWasm } from "./wasm/loader";
import { buildVaultTree } from "./tree/builder";
import { formatTreeOutput } from "./tree/renderer";
import { McpServer } from "./mcp/server";
import { PublishPreviewModal } from "./publish/preview";
import { TriageModal } from "./organize/triage-modal";
import {
  VaultTreeSettings,
  DEFAULT_SETTINGS,
  VaultTreeSettingTab,
} from "./settings";

export default class VaultTreePlugin extends Plugin {
  settings: VaultTreeSettings = DEFAULT_SETTINGS;
  mcpServer: McpServer | null = null;

  async onload(): Promise<void> {
    await this.loadSettings();

    // Initialize WASM module
    try {
      await initWasm();
      console.log("Vault Tree: WASM module initialized");
    } catch (error) {
      console.error("Vault Tree: Failed to initialize WASM", error);
      new Notice("Vault Tree: Failed to initialize WASM module");
    }

    // Start MCP server if enabled
    if (this.settings.enableMcpServer) {
      try {
        this.mcpServer = new McpServer(this.app, () => this.settings);
        await this.mcpServer.start();
        console.log("Vault Tree: MCP server started");
      } catch (error) {
        console.error("Vault Tree: Failed to start MCP server", error);
      }
    }

    // Tree commands
    this.addCommand({
      id: "copy-vault-tree",
      name: "Copy vault tree to clipboard",
      callback: () => this.copyTreeToClipboard(),
    });

    this.addCommand({
      id: "insert-vault-tree",
      name: "Insert vault tree at cursor",
      editorCallback: (editor: Editor) => this.insertTreeAtCursor(editor),
    });

    this.addCommand({
      id: "show-vault-tree",
      name: "Show vault tree in modal",
      callback: () => this.showTreeModal(),
    });

    // Publish commands
    this.addCommand({
      id: "publish-current-file",
      name: "Publish current file to droo.foo",
      checkCallback: (checking: boolean) => {
        const file = this.app.workspace.getActiveFile();
        if (file && file.extension === "md") {
          if (!checking) {
            this.publishCurrentFile();
          }
          return true;
        }
        return false;
      },
    });

    // Organization commands
    this.addCommand({
      id: "triage-inbox",
      name: "Triage inbox notes",
      callback: () => this.openTriageModal(),
    });

    // Add settings tab
    this.addSettingTab(new VaultTreeSettingTab(this.app, this));
  }

  async onunload(): Promise<void> {
    if (this.mcpServer) {
      await this.mcpServer.stop();
      this.mcpServer = null;
    }
  }

  async loadSettings(): Promise<void> {
    this.settings = Object.assign({}, DEFAULT_SETTINGS, await this.loadData());
  }

  async saveSettings(): Promise<void> {
    await this.saveData(this.settings);
  }

  private async generateTree(): Promise<string> {
    const wasm = getWasm();
    if (!wasm) {
      throw new Error("WASM module not initialized");
    }

    const depth = this.settings.defaultDepth || undefined;
    const result = await buildVaultTree(this.app, { depth });
    return formatTreeOutput(result);
  }

  private async copyTreeToClipboard(): Promise<void> {
    try {
      const tree = await this.generateTree();
      await navigator.clipboard.writeText(tree);
      new Notice("Vault tree copied to clipboard");
    } catch (error) {
      console.error("Failed to copy tree:", error);
      new Notice("Failed to copy vault tree");
    }
  }

  private async insertTreeAtCursor(editor: Editor): Promise<void> {
    try {
      const tree = await this.generateTree();
      const formatted = "```\n" + tree + "```";
      editor.replaceSelection(formatted);
    } catch (error) {
      console.error("Failed to insert tree:", error);
      new Notice("Failed to insert vault tree");
    }
  }

  private async showTreeModal(): Promise<void> {
    try {
      const tree = await this.generateTree();
      new TreeModal(this.app, tree).open();
    } catch (error) {
      console.error("Failed to show tree:", error);
      new Notice("Failed to generate vault tree");
    }
  }

  private async publishCurrentFile(): Promise<void> {
    const file = this.app.workspace.getActiveFile();
    if (!file) {
      new Notice("No file is currently open");
      return;
    }

    try {
      const content = await this.app.vault.read(file);
      new PublishPreviewModal(this.app, this, file, content).open();
    } catch (error) {
      console.error("Failed to open publish dialog:", error);
      new Notice("Failed to open publish dialog");
    }
  }

  private openTriageModal(): void {
    const organizeSettings = {
      inboxFolder: this.settings.inboxFolder,
      excludeFolders: this.settings.excludeFolders,
      minConfidence: this.settings.minConfidence,
      autoGenerateFrontmatter: this.settings.autoGenerateFrontmatter,
    };

    new TriageModal(this.app, organizeSettings).open();
  }
}

class TreeModal extends Modal {
  private tree: string;

  constructor(app: App, tree: string) {
    super(app);
    this.tree = tree;
  }

  onOpen(): void {
    const { contentEl } = this;

    contentEl.createEl("h2", { text: "Vault Tree" });

    const pre = contentEl.createEl("pre", {
      cls: "vault-tree-output",
    });
    pre.createEl("code", { text: this.tree });

    const buttonContainer = contentEl.createDiv({ cls: "vault-tree-buttons" });

    const copyButton = buttonContainer.createEl("button", { text: "Copy to Clipboard" });
    copyButton.addEventListener("click", async () => {
      await navigator.clipboard.writeText(this.tree);
      new Notice("Copied to clipboard");
    });

    const closeButton = buttonContainer.createEl("button", { text: "Close" });
    closeButton.addEventListener("click", () => this.close());
  }

  onClose(): void {
    const { contentEl } = this;
    contentEl.empty();
  }
}
