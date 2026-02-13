import { App, Modal, Setting, Notice, TFile } from "obsidian";
import type { PostMetadata, ValidationResult, PostStats } from "./types";
import {
  validateMetadata,
  extractMetadataFromFrontmatter,
  calculatePostStats,
  generateSlug,
  getTodayDate,
} from "./validator";
import { publishPost, updatePost, checkPostExists } from "./bridge";
import type VaultTreePlugin from "../main";

export class PublishPreviewModal extends Modal {
  private plugin: VaultTreePlugin;
  private file: TFile;
  private content: string;
  private metadata: Partial<PostMetadata>;
  private stats: PostStats;
  private validation: ValidationResult;

  constructor(app: App, plugin: VaultTreePlugin, file: TFile, content: string) {
    super(app);
    this.plugin = plugin;
    this.file = file;
    this.content = content;
    this.metadata = extractMetadataFromFrontmatter(content) || {};
    this.stats = calculatePostStats(content);
    this.validation = validateMetadata(this.metadata);
  }

  onOpen(): void {
    const { contentEl } = this;
    contentEl.empty();
    contentEl.addClass("vault-tree-publish-modal");

    contentEl.createEl("h2", { text: "Publish to droo.foo" });

    // File info
    const fileInfo = contentEl.createDiv({ cls: "publish-file-info" });
    fileInfo.createEl("p", { text: `File: ${this.file.path}` });

    // Stats
    const statsEl = contentEl.createDiv({ cls: "publish-stats" });
    statsEl.createEl("span", { text: `${this.stats.wordCount} words` });
    statsEl.createEl("span", { text: " | " });
    statsEl.createEl("span", { text: this.stats.estimatedReadingTime });
    statsEl.createEl("span", { text: " | " });
    statsEl.createEl("span", { text: `${this.stats.linkCount} links` });

    // Validation status
    if (!this.validation.valid) {
      const errorsEl = contentEl.createDiv({ cls: "publish-errors" });
      errorsEl.createEl("h3", { text: "Errors" });
      for (const error of this.validation.errors) {
        errorsEl.createEl("p", {
          text: `${error.field}: ${error.message}`,
          cls: "publish-error",
        });
      }
    }

    if (this.validation.warnings.length > 0) {
      const warningsEl = contentEl.createDiv({ cls: "publish-warnings" });
      warningsEl.createEl("h3", { text: "Warnings" });
      for (const warning of this.validation.warnings) {
        warningsEl.createEl("p", {
          text: `${warning.field}: ${warning.message}`,
          cls: "publish-warning",
        });
      }
    }

    // Metadata fields
    const metadataEl = contentEl.createDiv({ cls: "publish-metadata" });
    metadataEl.createEl("h3", { text: "Metadata" });

    new Setting(metadataEl)
      .setName("Title")
      .setDesc("The post title")
      .addText((text) =>
        text
          .setValue(this.metadata.title || "")
          .onChange((value) => {
            this.metadata.title = value;
            this.updateValidation();
          })
      );

    new Setting(metadataEl)
      .setName("Slug")
      .setDesc("URL-friendly identifier")
      .addText((text) =>
        text
          .setValue(this.metadata.slug || "")
          .setPlaceholder(generateSlug(this.metadata.title || ""))
          .onChange((value) => {
            this.metadata.slug = value;
            this.updateValidation();
          })
      )
      .addButton((button) =>
        button.setButtonText("Generate").onClick(() => {
          this.metadata.slug = generateSlug(this.metadata.title || "");
          this.refresh();
        })
      );

    new Setting(metadataEl)
      .setName("Date")
      .setDesc("Publication date (YYYY-MM-DD)")
      .addText((text) =>
        text
          .setValue(this.metadata.date || "")
          .setPlaceholder(getTodayDate())
          .onChange((value) => {
            this.metadata.date = value;
            this.updateValidation();
          })
      )
      .addButton((button) =>
        button.setButtonText("Today").onClick(() => {
          this.metadata.date = getTodayDate();
          this.refresh();
        })
      );

    new Setting(metadataEl)
      .setName("Description")
      .setDesc("Short description for SEO and previews")
      .addTextArea((text) =>
        text
          .setValue(this.metadata.description || "")
          .onChange((value) => {
            this.metadata.description = value;
            this.updateValidation();
          })
      );

    new Setting(metadataEl)
      .setName("Tags")
      .setDesc("Comma-separated tags")
      .addText((text) =>
        text
          .setValue((this.metadata.tags || []).join(", "))
          .onChange((value) => {
            this.metadata.tags = value
              .split(",")
              .map((t) => t.trim())
              .filter((t) => t.length > 0);
            this.updateValidation();
          })
      );

    new Setting(metadataEl)
      .setName("Author")
      .setDesc("Author name (optional)")
      .addText((text) =>
        text
          .setValue(this.metadata.author || "")
          .setPlaceholder("DROO AMOR")
          .onChange((value) => {
            this.metadata.author = value || undefined;
          })
      );

    new Setting(metadataEl)
      .setName("Series")
      .setDesc("Series name (optional)")
      .addText((text) =>
        text.setValue(this.metadata.series || "").onChange((value) => {
          this.metadata.series = value || undefined;
        })
      );

    new Setting(metadataEl)
      .setName("Series Order")
      .setDesc("Position in series (optional)")
      .addText((text) =>
        text
          .setValue(this.metadata.series_order?.toString() || "")
          .onChange((value) => {
            const num = parseInt(value, 10);
            this.metadata.series_order = isNaN(num) ? undefined : num;
          })
      );

    // Buttons
    const buttonContainer = contentEl.createDiv({ cls: "publish-buttons" });

    const cancelButton = buttonContainer.createEl("button", { text: "Cancel" });
    cancelButton.addEventListener("click", () => this.close());

    const publishButton = buttonContainer.createEl("button", {
      text: "Publish",
      cls: "mod-cta",
    });
    publishButton.disabled = !this.validation.valid;
    publishButton.addEventListener("click", () => this.doPublish());
  }

  onClose(): void {
    const { contentEl } = this;
    contentEl.empty();
  }

  private updateValidation(): void {
    this.validation = validateMetadata(this.metadata);
  }

  private refresh(): void {
    this.updateValidation();
    this.onOpen();
  }

  private async doPublish(): Promise<void> {
    if (!this.validation.valid) {
      new Notice("Please fix validation errors before publishing");
      return;
    }

    const apiToken = this.plugin.settings.apiToken;
    if (!apiToken) {
      new Notice("API token not configured. Set it in plugin settings.");
      return;
    }

    const metadata = this.metadata as PostMetadata;
    const options = {
      apiToken,
      apiUrl: this.plugin.settings.apiUrl || undefined,
    };

    // Check if post already exists
    const exists = await checkPostExists(metadata.slug, options);

    let result;
    if (exists) {
      const confirm = await this.confirmUpdate();
      if (!confirm) return;
      result = await updatePost(metadata.slug, this.content, metadata, options);
    } else {
      result = await publishPost(this.content, metadata, options);
    }

    if (result.success) {
      new Notice(`Published successfully! ${result.url}`);
      this.close();
    } else {
      new Notice(`Publish failed: ${result.error}`);
    }
  }

  private async confirmUpdate(): Promise<boolean> {
    return new Promise((resolve) => {
      const modal = new ConfirmModal(
        this.app,
        "Post already exists",
        "A post with this slug already exists. Do you want to update it?",
        () => resolve(true),
        () => resolve(false)
      );
      modal.open();
    });
  }
}

class ConfirmModal extends Modal {
  private title: string;
  private message: string;
  private onConfirm: () => void;
  private onCancel: () => void;

  constructor(
    app: App,
    title: string,
    message: string,
    onConfirm: () => void,
    onCancel: () => void
  ) {
    super(app);
    this.title = title;
    this.message = message;
    this.onConfirm = onConfirm;
    this.onCancel = onCancel;
  }

  onOpen(): void {
    const { contentEl } = this;
    contentEl.createEl("h2", { text: this.title });
    contentEl.createEl("p", { text: this.message });

    const buttonContainer = contentEl.createDiv({ cls: "modal-button-container" });

    const cancelButton = buttonContainer.createEl("button", { text: "Cancel" });
    cancelButton.addEventListener("click", () => {
      this.onCancel();
      this.close();
    });

    const confirmButton = buttonContainer.createEl("button", {
      text: "Update",
      cls: "mod-warning",
    });
    confirmButton.addEventListener("click", () => {
      this.onConfirm();
      this.close();
    });
  }

  onClose(): void {
    const { contentEl } = this;
    contentEl.empty();
  }
}
