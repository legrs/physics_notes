#!/usr/bin/env node
// Full local test suite — one command, all phases:
//
//   npm test              (or: node scripts/tests/run_all.js)
//
// Phases (each exits non-zero on failure):
//   1. html_syntax — inline-script parse, SRI pinning, required ids
//   2. html_stress — the REAL search.html ranking functions, real corpus,
//      extreme-input battery, hybrid/RRF with real embedding vectors
//   3. data_check  — JSON schema, embeddings coverage, version.json hashes,
//      search_text regenerates deterministically
//   4. parity_check — web-vs-physq ranking parity (physq/scripts/parity_check.js)
//   5. physq — cargo fmt/clippy/test, debug build, then binary stress
//      (real searches on the local corpus, BM25-only, no model download)
//
// The GitHub Actions equivalent (run on every push) lives in
// .github/workflows/test.yml — this script mirrors its jobs locally.
'use strict';

const path = require('path');
const fs = require('fs');
const { spawnSync } = require('child_process');
const { REPO_ROOT } = require('./_extract');

const FAILS = [];

function run(label, cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { cwd: REPO_ROOT, stdio: 'inherit', ...opts });
  const ok = r.error ? false : r.status === 0;
  console.log(`\n${ok ? '✓' : '✗'} ${label}${ok ? '' : ` (${r.error ? r.error.message : 'exit ' + r.status})`}`);
  if (!ok) FAILS.push(label);
  return ok;
}

console.log('=== physics_notes full test suite (STRICT) ===');

// Phase 1–2: HTML
run('html syntax', process.execPath, [path.join('scripts', 'tests', 'html_syntax.js')]);
run('html stress', process.execPath, [path.join('scripts', 'tests', 'html_stress.js')]);

// Phase 2.5: QA images (UUID / licenses / normalize / search_text invariants)
run('qa images stress', process.execPath, [path.join('scripts', 'tests', 'qa_images_stress.js')]);
run('normalize --check', process.execPath, [path.join('scripts', 'normalize-images.js'), '--check']);

// Phase 3: data artifacts
run('data consistency', process.execPath, [path.join('scripts', 'tests', 'data_check.js')]);

// Phase 4: web-vs-physq parity (needs repo-root npm deps — npm ci first)
if (!fs.existsSync(path.join(REPO_ROOT, 'node_modules'))) {
  console.log('\nnote: repo-root node_modules missing — running npm ci');
  if (!run('npm ci', 'npm', ['ci'])) { console.error('npm ci failed'); process.exit(1); }
}
run('parity check', process.execPath, [path.join('physq', 'scripts', 'parity_check.js')]);

// Phase 5: physq (cargo checks inside physq/)
const physqDir = path.join(REPO_ROOT, 'physq');
run('cargo fmt --check', 'cargo', ['fmt', '--check'], { cwd: physqDir });
run('cargo clippy', 'cargo', ['clippy'], { cwd: physqDir });
run('cargo test', 'cargo', ['test'], { cwd: physqDir });
run('physq binary stress', process.execPath, [path.join('scripts', 'tests', 'run_physq.js')]);

console.log(
  FAILS.length === 0
    ? '\nALL TEST PHASES PASSED'
    : `\nFAILED PHASES: ${FAILS.join(', ')}`
);
process.exit(FAILS.length === 0 ? 0 : 1);