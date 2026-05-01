#!/usr/bin/env node
// Preprocess JetBrains color scheme XMLs into a compact themes.json.
// Run once by a maintainer when adding/updating bundled themes.
//
// Usage: node scripts/build-themes.js [path-to-xml-dir]
// Default path: ~/projects/samples/jetbrains/colors

const fs = require("fs");
const path = require("path");
const os = require("os");

const srcDir = process.argv[2]
  || path.join(os.homedir(), "projects/samples/jetbrains/colors");
const outFile = path.join(__dirname, "..", "themes.json");

function hex(v) {
  if (!v) return null;
  return "#" + v.padStart(6, "0").toLowerCase();
}

function extract(xml) {
  const colorsSection = xml.match(/<colors>([\s\S]*?)<\/colors>/)?.[1] || "";
  const colors = {};
  const colorRe = /<option\s+name="([^"]+)"\s+value="([^"]*)"\s*\/>/g;
  let m;
  while ((m = colorRe.exec(colorsSection))) colors[m[1]] = m[2];

  const attrsSection = xml.match(/<attributes>([\s\S]*?)<\/attributes>/)?.[1] || "";
  const attrs = {};
  const blockRe =
    /<option\s+name="([^"]+)"[^>]*>\s*<value>([\s\S]*?)<\/value>\s*<\/option>/g;
  while ((m = blockRe.exec(attrsSection))) {
    const inner = {};
    const innerRe = /<option\s+name="([^"]+)"\s+value="([^"]*)"\s*\/>/g;
    let im;
    while ((im = innerRe.exec(m[2]))) inner[im[1]] = im[2];
    attrs[m[1]] = inner;
  }

  const nameMatch = xml.match(/<scheme\s+name="([^"]+)"/);
  const text = attrs.TEXT || {};

  return {
    name: nameMatch?.[1] || "",
    editorBg: hex(text.BACKGROUND),
    editorFg: hex(text.FOREGROUND),
    chrome: {
      lineNumbers: hex(colors.LINE_NUMBERS_COLOR),
      gutter: hex(colors.GUTTER_BACKGROUND),
      caret: hex(colors.CARET_COLOR),
      caretRow: hex(colors.CARET_ROW_COLOR),
      selection: hex(colors.SELECTION_BACKGROUND),
      added: hex(colors.ADDED_LINES_COLOR || colors.FILESTATUS_ADDED),
      deleted: hex(colors.DELETED_LINES_COLOR || colors.FILESTATUS_DELETED),
      modified: hex(colors.MODIFIED_LINES_COLOR || colors.FILESTATUS_MODIFIED),
      separator: hex(colors.METHOD_SEPARATORS_COLOR || colors.INDENT_GUIDE),
    },
    tokens: {
      keyword: attrs.DEFAULT_KEYWORD || null,
      string: attrs.DEFAULT_STRING || null,
      number: attrs.DEFAULT_NUMBER || null,
      comment:
        attrs.DEFAULT_LINE_COMMENT || attrs.DEFAULT_BLOCK_COMMENT || null,
      function: attrs.DEFAULT_FUNCTION_DECLARATION || null,
      type: attrs.DEFAULT_CLASS_NAME || null,
      operator: attrs.DEFAULT_OPERATION_SIGN || null,
    },
  };
}

if (!fs.existsSync(srcDir)) {
  console.error(`Source directory not found: ${srcDir}`);
  process.exit(1);
}

const files = fs.readdirSync(srcDir).filter((f) => f.endsWith(".xml")).sort();
const out = {};
let skipped = 0;
for (const f of files) {
  const id = f.slice(0, -4);
  try {
    const xml = fs.readFileSync(path.join(srcDir, f), "utf-8");
    out[id] = extract(xml);
  } catch (e) {
    console.warn(`skip ${id}: ${e.message}`);
    skipped++;
  }
}

fs.writeFileSync(outFile, JSON.stringify(out));
const sizeKB = (fs.statSync(outFile).size / 1024).toFixed(1);
console.log(
  `wrote ${outFile}: ${Object.keys(out).length} themes, ${sizeKB} KB` +
    (skipped ? ` (${skipped} skipped)` : ""),
);
