#!/usr/bin/env node
// Local physq CLI stress + smoke harness (used by `npm run test:physq` and by
// run_all.js). This is the same battery the CI matrix runs, expressed in
// Node so it works on any OS (Linux/macOS/Windows) without bash quirks.
//
//   1. Ensures a debug binary exists (builds it if missing).
//   2. Real searches via the REAL engine on the LOCAL corpus:
//        physq --model none eval --data q_and_a_data.json \
//              --cases scripts/tests/physq_stress_cases.jsonl
//      (`--model none` = BM25-only — no ~470MB model download.)
//      Asserts: exit 0, exactly N result lines + 1 summary, no `panicked`.
//   3. Edge handling: malformed JSONL and unknown flags must fail gracefully
//      (soft error / clap usage), never crash.
//
// Usage: node scripts/tests/run_physq.js
'use strict';

const fs = require('fs');
const path = require('path');
const os = require('os');
const { spawnSync, execFileSync } = require('child_process');
const { REPO_ROOT } = require('./_extract');

function buildBinary() {
  const exe = process.platform === 'win32' ? 'physq.exe' : 'physq';
  // Respect matrix TARGET if set (CI physq job builds --target <triple> to target/<triple>/debug)
  const target = process.env.TARGET || '';
  const candidates = [];
  if (target) candidates.push(path.join(REPO_ROOT, 'physq', 'target', target, 'debug', exe));
  candidates.push(path.join(REPO_ROOT, 'physq', 'target', 'debug', exe));
  for (const p of candidates) if (fs.existsSync(p)) return p;
  process.stdout.write('physq debug binary not found — building (cargo build)…\n');
  const args = target ? ['build', '--target', target] : ['build'];
  const r = spawnSync('cargo', args, { cwd: path.join(REPO_ROOT, 'physq'), stdio: 'inherit' });
  if (r.status !== 0) {
    console.error('FAIL: cargo build of physq failed');
    process.exit(r.status ?? 1);
  }
  for (const p of candidates) if (fs.existsSync(p)) return p;
  console.error(`FAIL: binary not produced at ${candidates.join(' or ')}`);
  process.exit(1);
}

const bin = buildBinary();
const runBin = (args, opts = {}) =>
  spawnSync(bin, args, { cwd: REPO_ROOT, encoding: 'utf-8', ...opts });

let failures = 0;
let checks = 0;
function ok(cond, msg) {
  checks++;
  if (!cond) { failures++; console.log(`FAIL ${msg}`); }
  else console.log(`PASS ${msg}`);
}

// ── 1. runs on the host ────────────────────────────────────────────────
const ver = runBin(['--version']);
ok(ver.status === 0 && /\d+\.\d+\.\d+/.test(ver.stdout), `--version prints a semver (${ver.stdout.trim()})`);
ok(runBin(['--help']).status === 0, '--help exits 0');
const cp = runBin(['cache', 'path']);
ok(cp.status === 0 && cp.stdout.trim().length > 0, 'cache path prints a directory');

// ── 2. real searches / stress battery ──────────────────────────────────
const cases = path.join(REPO_ROOT, 'scripts', 'tests', 'physq_stress_cases.jsonl');
const data = path.join(REPO_ROOT, 'q_and_a_data.json');
const stress = runBin(['--model', 'none', 'eval', '--data', data, '--cases', cases]);
const lines = (stress.stdout || '').trim().split('\n').filter(Boolean);
const nCases = fs.readFileSync(cases, 'utf-8').trim().split('\n').filter(Boolean).length;
ok(stress.status === 0, `eval stress exit 0 (got ${stress.status})`);
ok(lines.length === nCases + 1, `eval stress emitted ${nCases} results + 1 summary (got ${lines.length})`);
ok(lines.some((l) => l.includes('"type":"summary"')), 'eval stress emitted a summary line');
ok(!(stress.stdout + stress.stderr).includes('panicked'), 'no Rust panic anywhere in stress output');
ok((stress.stderr || '').includes('loading local data'), 'eval used the local corpus');
const errors = lines.filter((l) => l.includes('"type":"error"'));
ok(errors.length <= 2, `at most the surrogate-escape cases soft-error (${errors.length} errors)`);

// ── 3. malformed input & flags handled gracefully ──────────────────────
const bad = path.join(os.tmpdir(), `physq-bad-${process.pid}.jsonl`);
fs.writeFileSync(bad, 'not json\n{"query":"電気"}\n');
const softErr = runBin(['--model', 'none', 'eval', '--data', data, '--cases', bad]);
ok(softErr.status === 0 && softErr.stdout.includes('"type":"summary"'), 'malformed JSONL → soft error + summary, exit 0');
fs.unlinkSync(bad);

const bogus = runBin(['--no-such-flag']);
ok(bogus.status !== 0, `unknown flag exits non-zero (got ${bogus.status})`);
ok((bogus.stderr || '').toLowerCase().includes('error'), 'unknown flag reports a clap usage error');
ok(!(bogus.stderr || '').includes('panicked'), 'unknown flag does not panic');

const missing = runBin(['--model', 'none', 'eval', '--data', data, '--cases', '/no/such/file.jsonl']);
ok(missing.status !== 0, 'missing cases file exits non-zero (no crash)');

console.log(`\n${checks} checks, ${failures} failures`);
console.log(failures === 0 ? 'PHYSQ STRESS TESTS PASSED' : 'PHYSQ STRESS TESTS FAILED');
process.exit(failures === 0 ? 0 : 1);