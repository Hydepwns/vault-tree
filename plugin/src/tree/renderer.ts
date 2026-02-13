import type { TreeResult } from "../wasm/types";

export function formatTreeOutput(result: TreeResult): string {
  let output = result.tree;

  if (!output.endsWith("\n")) {
    output += "\n";
  }

  output += `\n${result.total_notes} notes, ${result.total_dirs} directories\n`;

  return output;
}

export function formatTreeForClipboard(result: TreeResult): string {
  return "```\n" + formatTreeOutput(result) + "```";
}
