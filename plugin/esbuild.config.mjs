import esbuild from "esbuild";
import fs from "fs";
import path from "path";

const prod = process.argv[2] === "production";

// Read WASM as base64
const wasmPath = path.join(import.meta.dirname, "wasm", "vault_tree_wasm_bg.wasm");
const wasmBase64 = fs.readFileSync(wasmPath).toString("base64");

// Read WASM JS bindings
const wasmJsPath = path.join(import.meta.dirname, "wasm", "vault_tree_wasm.js");
let wasmJs = fs.readFileSync(wasmJsPath, "utf-8");

// Modify WASM JS to use inline base64 instead of fetch
wasmJs = wasmJs.replace(
  /input = new URL\([^)]+\);/,
  `input = Uint8Array.from(atob("${wasmBase64}"), c => c.charCodeAt(0));`
);

// Write modified WASM JS for bundling
const modifiedWasmJsPath = path.join(import.meta.dirname, "wasm", "vault_tree_wasm_inline.js");
fs.writeFileSync(modifiedWasmJsPath, wasmJs);

const context = await esbuild.context({
  entryPoints: ["src/main.ts"],
  bundle: true,
  external: ["obsidian", "electron"],
  format: "cjs",
  target: "es2022",
  logLevel: "info",
  sourcemap: prod ? false : "inline",
  treeShaking: true,
  outfile: "main.js",
  minify: prod,
  define: {
    "process.env.NODE_ENV": prod ? '"production"' : '"development"',
  },
});

if (prod) {
  await context.rebuild();
  process.exit(0);
} else {
  await context.watch();
}
