import { App, Modal, Setting, Notice, TFolder } from "obsidian";
import type { TriageItem, OrganizeSettings } from "./types";
import { triageInbox, applyTriageDecisions, formatTriageResult } from "./triage";

export class TriageModal extends Modal {
  private settings: OrganizeSettings;
  private items: TriageItem[] = [];
  private folders: string[] = [];

  constructor(app: App, settings: OrganizeSettings) {
    super(app);
    this.settings = settings;
  }

  async onOpen(): Promise<void> {
    const { contentEl } = this;
    contentEl.empty();
    contentEl.addClass("vault-tree-triage-modal");

    contentEl.createEl("h2", { text: "Triage Inbox Notes" });

    // Loading state
    const loadingEl = contentEl.createDiv({ cls: "triage-loading" });
    loadingEl.createEl("p", { text: "Analyzing inbox..." });

    // Load suggestions
    this.items = await triageInbox(this.app, this.settings);
    this.folders = this.getAllFolders();

    loadingEl.remove();

    if (this.items.length === 0) {
      contentEl.createEl("p", { text: "No files found in inbox folder." });
      return;
    }

    // Summary
    const summaryEl = contentEl.createDiv({ cls: "triage-summary" });
    summaryEl.createEl("p", {
      text: `Found ${this.items.length} files in ${this.settings.inboxFolder}`,
    });

    // Quick actions
    const actionsEl = contentEl.createDiv({ cls: "triage-actions" });

    const acceptAllBtn = actionsEl.createEl("button", { text: "Accept All" });
    acceptAllBtn.addEventListener("click", () => {
      for (const item of this.items) {
        if (item.suggestion.confidence >= this.settings.minConfidence) {
          item.status = "accepted";
        }
      }
      this.renderItems();
    });

    const rejectAllBtn = actionsEl.createEl("button", { text: "Reject All" });
    rejectAllBtn.addEventListener("click", () => {
      for (const item of this.items) {
        item.status = "rejected";
      }
      this.renderItems();
    });

    const resetBtn = actionsEl.createEl("button", { text: "Reset" });
    resetBtn.addEventListener("click", () => {
      for (const item of this.items) {
        item.status = "pending";
        item.modifiedFolder = undefined;
      }
      this.renderItems();
    });

    // Items container
    const itemsContainer = contentEl.createDiv({ cls: "triage-items" });
    this.renderItemsInto(itemsContainer);

    // Apply button
    const buttonContainer = contentEl.createDiv({ cls: "triage-buttons" });

    const cancelBtn = buttonContainer.createEl("button", { text: "Cancel" });
    cancelBtn.addEventListener("click", () => this.close());

    const applyBtn = buttonContainer.createEl("button", {
      text: "Apply Changes",
      cls: "mod-cta",
    });
    applyBtn.addEventListener("click", () => this.applyChanges());
  }

  onClose(): void {
    const { contentEl } = this;
    contentEl.empty();
  }

  private renderItems(): void {
    const container = this.contentEl.querySelector(".triage-items");
    if (container) {
      container.empty();
      this.renderItemsInto(container as HTMLElement);
    }
  }

  private renderItemsInto(container: HTMLElement): void {
    for (const item of this.items) {
      this.renderItem(container, item);
    }
  }

  private renderItem(container: HTMLElement, item: TriageItem): void {
    const itemEl = container.createDiv({ cls: `triage-item triage-${item.status}` });

    // File name
    const fileName = item.file.split("/").pop() || item.file;
    itemEl.createEl("h4", { text: fileName });

    // Suggestion info
    const suggestionEl = itemEl.createDiv({ cls: "triage-suggestion" });
    const confidence = (item.suggestion.confidence * 100).toFixed(0);

    if (item.suggestion.suggestedFolder) {
      suggestionEl.createEl("span", {
        text: `Suggested: ${item.suggestion.suggestedFolder} (${confidence}%)`,
      });
    } else {
      suggestionEl.createEl("span", {
        text: "No suggestion available",
        cls: "triage-no-suggestion",
      });
    }

    // Reasons
    if (item.suggestion.reasons.length > 0) {
      const reasonsEl = itemEl.createDiv({ cls: "triage-reasons" });
      for (const reason of item.suggestion.reasons) {
        reasonsEl.createEl("span", { text: reason, cls: "triage-reason" });
      }
    }

    // Decision controls
    const controlsEl = itemEl.createDiv({ cls: "triage-controls" });

    // Status buttons
    const acceptBtn = controlsEl.createEl("button", {
      text: "Accept",
      cls: item.status === "accepted" ? "is-active" : "",
    });
    acceptBtn.addEventListener("click", () => {
      item.status = "accepted";
      item.modifiedFolder = undefined;
      this.renderItems();
    });

    const rejectBtn = controlsEl.createEl("button", {
      text: "Reject",
      cls: item.status === "rejected" ? "is-active" : "",
    });
    rejectBtn.addEventListener("click", () => {
      item.status = "rejected";
      this.renderItems();
    });

    // Folder override dropdown
    const folderSelect = controlsEl.createEl("select");
    folderSelect.createEl("option", { value: "", text: "-- Change folder --" });

    for (const folder of this.folders) {
      const opt = folderSelect.createEl("option", { value: folder, text: folder });
      if (item.modifiedFolder === folder) {
        opt.selected = true;
      }
    }

    folderSelect.addEventListener("change", () => {
      if (folderSelect.value) {
        item.status = "modified";
        item.modifiedFolder = folderSelect.value;
      } else if (item.status === "modified") {
        item.status = "pending";
        item.modifiedFolder = undefined;
      }
      this.renderItems();
    });
  }

  private getAllFolders(): string[] {
    const folders: string[] = [];

    const collectFolders = (folder: TFolder, path: string) => {
      if (
        !this.settings.excludeFolders.some((ex) => path.startsWith(ex)) &&
        path !== this.settings.inboxFolder
      ) {
        folders.push(path);
      }

      for (const child of folder.children) {
        if (child instanceof TFolder) {
          collectFolders(child, child.path);
        }
      }
    };

    collectFolders(this.app.vault.getRoot(), "");
    return folders.filter((f) => f.length > 0).sort();
  }

  private async applyChanges(): Promise<void> {
    const toProcess = this.items.filter((i) => i.status !== "pending");

    if (toProcess.length === 0) {
      new Notice("No decisions to apply");
      return;
    }

    const result = await applyTriageDecisions(this.app, this.items);

    new Notice(
      `Triage complete: ${result.accepted} accepted, ${result.modified} modified, ${result.rejected} rejected`
    );

    this.close();
  }
}
