#!/usr/bin/env node
// HTML syntax / structural health check for every page in the repo.
//
// Checks, per HTML file, without a browser:
//   1. Every inline <script> block parses (node --check semantics). A broken
//      script would take down the whole page (one syntax error blanks the
//      entire inline script that contains it).
//   2. <script> / </script> tag balance.
//   3. Basic document skeleton: <!DOCTYPE html>, <html>, </html>, <title>.
//   4. Required functional element IDs that the app's JS reads via
//      getElementById — if one is renamed/deleted, the page silently breaks.
//   5. CDN scripts loaded with a URL must carry a pinned SRI `integrity` hash
//      plus `crossorigin` (repo policy: CDN assets are never vendored and
//      must stay pinned — see root LICENSE/NOTICE and CLAUDE.md).
//
// Usage: node scripts/tests/html_syntax.js
'use strict';

const { loadHtml, extractScripts, syntaxError } = require('./_extract');

const PAGES = [
  'index.html',
  'search.html',
  'debug_search.html',
  'qa_editor.html',
  'fast/index.html',
  'fast/search.html',
];

// App code reads these ids (verified against the page sources). Missing ones
// break interactions silently. debug_search.html is the standalone debug/dev
// copy and names its controls differently (search-input/search-btn).
const REQUIRED_IDS = {
  'search.html': ['search', 'results', 'semantic-status', 'semantic-status-text'],
  'debug_search.html': ['search-input', 'search-btn', 'status-bar'],
  'index.html': ['search'],
};

let failures = 0;
function ok(cond, msg) {
  if (!cond) failures++;
  console.log(`${cond ? 'PASS' : 'FAIL'} ${msg}`);
}

for (const page of PAGES) {
  const html = loadHtml(page);

  ok(html.includes('<!DOCTYPE'), `${page}: DOCTYPE present`);
  ok(html.includes('<html'), `${page}: <html> present`);
  ok(html.trimEnd().endsWith('</html>'), `${page}: closes with </html>`);
  ok(/<title>\s*[^<]+\s*<\/title>/.test(html), `${page}: non-empty <title>`);

  const scripts = extractScripts(html);
  const inlineScripts = scripts.filter((s) => !s.src);
  const externScripts = scripts.filter((s) => s.src);

  // Script-tag balance. A naive count of `<script` text is NOT valid: a
  // `<script` inside a JS comment/string inside an actual script element is
  // inert for the HTML tokenizer (only `</script` terminates in script-data
  // state), whereas an unclosed `<script>` IS a real breakage. So remove the
  // spans the extractor actually matched and require nothing script-like to
  // be left over — leftovers mean a genuine orphan open/close tag.
  let remainder = html;
  scripts
    .slice()
    .sort((a, b) => b.start - a.start)
    .forEach((s) => { remainder = remainder.slice(0, s.start) + remainder.slice(s.end); });
  const stray = remainder.match(/<\/?script\b/gi) || [];
  ok(stray.length === 0, `${page}: no stray/unbalanced script tags (${stray.length} leftover)`);

  inlineScripts.forEach((s, i) => {
    const err = syntaxError(s.code, false);
    ok(err === null, `${page}: inline <script> #${i} parses${err ? ` — ${err}` : ''}`);
  });

  const ids = new Set([...html.matchAll(/\bid="([^"]+)"/g)].map((m) => m[1]));
  for (const req of REQUIRED_IDS[page] || []) {
    ok(ids.has(req), `${page}: required id "#${req}" present`);
  }

  for (const s of externScripts) {
    const isCdn = s.src.startsWith('https://');
    if (isCdn) {
      ok(/\bintegrity\s*=/.test(s.tag), `${page}: CDN script "${s.src}" pins SRI integrity`);
      ok(/\bcrossorigin\s*=/.test(s.tag), `${page}: CDN script "${s.src}" sets crossorigin`);
    } else {
      ok(false, `${page}: external script "${s.src}" is not https`);
    }
  }
}

console.log(failures === 0 ? '\nHTML SYNTAX CHECKS PASSED' : `\n${failures} HTML SYNTAX FAILURES`);
process.exit(failures === 0 ? 0 : 1);