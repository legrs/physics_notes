#!/usr/bin/env node
// Shared helpers for the headless HTML test harness.
//
// search.html is a single static page: its ranking logic lives in plain
// `function _name(...) {...}` declarations inside an inline <script>. We
// extract those functions as text (brace matching — robust to edits
// elsewhere in the page, the same technique `physq/scripts/parity_check.js`
// uses) and eval them into a plain Node context, so the REAL page code can
// be driven without a browser, worker, or npm deps.
//
// This module is dependency-free (Node stdlib only).
'use strict';

const fs = require('fs');
const path = require('path');

const REPO_ROOT = path.resolve(__dirname, '..', '..');
const SEARCH_HTML = path.join(REPO_ROOT, 'search.html');

function loadHtml(rel) {
  return fs.readFileSync(path.join(REPO_ROOT, rel), 'utf-8');
}

function loadSearchHtml() {
  return loadHtml('search.html');
}

// `function <name>(...) { ... }` via brace matching. Throws if not found.
function extractFunction(html, name) {
  const start = html.indexOf(`function ${name}(`);
  if (start < 0) throw new Error(`function ${name} not found in search.html`);
  let i = html.indexOf('{', start);
  if (i < 0) throw new Error(`no body brace for function ${name}`);
  let depth = 0;
  for (; i < html.length; i++) {
    if (html[i] === '{') depth++;
    else if (html[i] === '}' && --depth === 0) break;
  }
  if (depth !== 0) throw new Error(`unbalanced braces for function ${name}`);
  return html.slice(start, i + 1);
}

// `const NAME = <brace-balanced value>;` — for object/array literals such as
// MODE_EMB_KEYS / SEMANTIC_MODELS (the RRF_WEIGHT_* are single lines and are
// handled by `constStatement`).
function extractConst(html, name) {
  const start = html.indexOf(`const ${name} = `);
  if (start < 0) throw new Error(`const ${name} not found in search.html`);
  const braceAt = html.indexOf('{', start);
  const useBrace = braceAt >= 0 && braceAt < html.indexOf(';', start);
  let end;
  if (useBrace) {
    let i = braceAt;
    let depth = 0;
    for (; i < html.length; i++) {
      if (html[i] === '{') depth++;
      else if (html[i] === '}' && --depth === 0) break;
    }
    const semi = html.indexOf(';', i);
    end = semi < 0 ? i + 1 : semi + 1;
  } else {
    const semi = html.indexOf(';', start);
    if (semi < 0) throw new Error(`const ${name} has no semicolon`);
    end = semi + 1;
  }
  return html.slice(start, end);
}

// Extract every <script> block (inline only; `<script src=...>` external
// tags are returned too but flagged with `src` so callers can skip body
// syntax-checking them). Multiline tag attrs are handled.
function extractScripts(html) {
  const out = [];
  const re = /<script\b([^>]*)>([\s\S]*?)<\/script>/gi;
  let m;
  while ((m = re.exec(html)) !== null) {
    const tag = `<script${m[1]}>`;
    const src = /src\s*=\s*["']([^"']+)["']/i.exec(m[1]);
    out.push({
      tag,
      src: src ? src[1] : null,
      code: m[2],
      start: m.index, // index of the opening <script
      end: re.lastIndex, // index just past </script>
    });
  }
  return out;
}

// Syntax-check a script body using `new Function` (fast, no temp files).
// Top-level `import`/`await` are NOT valid in a function body, so module
// scripts (type="module") must be passed with `asModule: true`, which routes
// them through `node --check` on a temp file instead.
function syntaxError(code, asModule = false) {
  if (asModule) {
    const os = require('os');
    const { execFileSync } = require('child_process');
    const tmp = path.join(os.tmpdir(), `psyn-${process.pid}-${Date.now()}.mjs`);
    try {
      fs.writeFileSync(tmp, code);
      execFileSync(process.execPath, ['--check', tmp], { stdio: 'pipe' });
      return null;
    } catch (e) {
      return String(e.stderr || e.message);
    } finally {
      try { fs.unlinkSync(tmp); } catch (_) { /* ignore */ }
    }
  }
  try {
    // eslint-disable-next-line no-new-func
    new Function(code);
    return null;
  } catch (e) {
    return String(e.message);
  }
}

// Eval a map of {name: sourceText} (functions and const statements) into one
// fresh closure and return a context of the named bindings.
function buildContext(srcMap) {
  const names = Object.keys(srcMap);
  const body = names.map((n) => srcMap[n]).join('\n');
  // eslint-disable-next-line no-eval
  const fn = eval(`(() => { ${body}
    return { ${names.join(', ')} }; })()`);
  return fn;
}

module.exports = {
  REPO_ROOT,
  SEARCH_HTML,
  loadHtml,
  loadSearchHtml,
  extractFunction,
  extractConst,
  extractScripts,
  syntaxError,
  buildContext,
};