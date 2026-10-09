import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

/**
 * Cache directory for the pre-bundled dependencies, keyed by the patched
 * dependency's content.
 *
 * `scripts/patch-gfm-autolink-lookbehind.mjs` rewrites
 * `mdast-util-gfm-autolink-literal` in place after install. Vite derives the
 * `?v=` hash of a pre-bundled dep from the lockfile, not from file contents,
 * so the URL would stay identical and WKWebView would keep serving its cached,
 * pre-patch `remark-gfm.js` — which still contains the lookbehind regex that
 * JavaScriptCore rejects, blanking the chat window. Folding the patched
 * content into `cacheDir` gives every patched revision its own path, so the
 * cached bundle can never be mistaken for the current one.
 */
function dependencyCacheDir(): string {
  try {
    const patched = readFileSync(
      "node_modules/mdast-util-gfm-autolink-literal/lib/index.js",
      "utf8",
    );
    return `node_modules/.vite-${createHash("sha256").update(patched).digest("hex").slice(0, 10)}`;
  } catch {
    return "node_modules/.vite";
  }
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  cacheDir: dependencyCacheDir(),

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
