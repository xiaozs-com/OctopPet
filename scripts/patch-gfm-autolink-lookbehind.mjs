// Re-applies a compatibility patch to `mdast-util-gfm-autolink-literal`.
//
// Why: its GFM email-autolink matcher uses a regex lookbehind
// `(?<=^|\s|\p{P}|\p{S})`. JavaScriptCore (WKWebView) only supports
// lookbehind from Safari 16.4 / macOS 13; on older macOS it throws
// `SyntaxError: Invalid regular expression: invalid group specifier name`,
// which crashes the chat window's React tree and renders a blank window.
//
// The lookbehind is redundant here: `findEmail` already re-validates the
// preceding character via `previous(match, true)`, so dropping the assertion
// keeps behaviour identical while compiling on older WebKit.
//
// Run automatically as a `postinstall` step, since `npm install` would
// otherwise restore the upstream file.

import { readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const LOOKBEHIND = "(?<=^|\\s|\\p{P}|\\p{S})";
const target = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "node_modules",
  "mdast-util-gfm-autolink-literal",
  "lib",
  "index.js",
);

if (!existsSync(target)) {
  console.log("[patch-gfm-autolink] package not installed, nothing to do");
  process.exit(0);
}

const source = await readFile(target, "utf8");

if (!source.includes(LOOKBEHIND)) {
  // Either already patched by us, or upstream changed shape. Only the first
  // is expected, so tell the user when the pattern disappears entirely.
  if (source.includes("@([-\\w]+(?:\\.[-\\w]+)+)")) {
    console.log("[patch-gfm-autolink] already patched");
  } else {
    console.warn(
      "[patch-gfm-autolink] expected content not found — the dependency " +
        "changed shape; re-check WebKit compatibility of this package.",
    );
  }
  process.exit(0);
}

await writeFile(target, source.replace(LOOKBEHIND, ""), "utf8");
console.log("[patch-gfm-autolink] removed unsupported regex lookbehind");
