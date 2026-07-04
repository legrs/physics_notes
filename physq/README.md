# physq — Physics Notes terminal search

A Rust TUI that searches the Physics Notes Q&A corpus **locally**, with the
web version's hybrid ranking: BM25 (with all field boosts) + multilingual-e5
semantic similarity, fused with RRF (k=60, semantic ×2.0).

The web's data artifacts are the source of truth: `physq` **fetches**
`q_and_a_data.json` and the pre-computed `embeddings.json` and never
recomputes corpus embeddings or re-tokenizes the corpus. Only the *query*
embedding is computed at runtime (fastembed / ONNX, no Python).

## Requirements & installation

Grab the archive for your platform from the
[Releases page](https://github.com/legrs/physics_notes/releases) and extract
it — `physq` is a single, self-contained binary (no runtime dependencies to
install separately; the ~470 MB embedding model downloads on first use, not
at install time).

| Platform | Requirement | Archive |
| --- | --- | --- |
| macOS | Apple Silicon (M1+); no Intel build (see below) | `physq-<version>-aarch64-apple-darwin.tar.gz` |
| Windows | x86_64 | `physq-<version>-x86_64-pc-windows-msvc.zip` |
| Linux | x86_64 or aarch64; **glibc ≥ 2.38** (Ubuntu 24.04+, Debian 13+, Fedora 39+, …) | `physq-<version>-{x86_64,aarch64}-unknown-linux-gnu.tar.gz` |

### macOS

```sh
curl -LO https://github.com/legrs/physics_notes/releases/download/physq-v<version>/physq-<version>-aarch64-apple-darwin.tar.gz
tar xzf physq-<version>-aarch64-apple-darwin.tar.gz
# unsigned binary: macOS Gatekeeper will refuse to run it until you clear the
# quarantine flag it sets on anything downloaded from a browser/curl
xattr -d com.apple.quarantine physq
sudo mv physq /usr/local/bin/   # or anywhere on your PATH
physq --version
```

### Linux

```sh
curl -LO https://github.com/legrs/physics_notes/releases/download/physq-v<version>/physq-<version>-x86_64-unknown-linux-gnu.tar.gz  # or the aarch64 archive
tar xzf physq-<version>-x86_64-unknown-linux-gnu.tar.gz
chmod +x physq
sudo mv physq /usr/local/bin/   # or anywhere on your PATH
physq --version
```

### Windows

1. Download `physq-<version>-x86_64-pc-windows-msvc.zip` and extract it.
2. Running `physq.exe` may trigger SmartScreen ("Windows protected your PC")
   since the binary is unsigned — click **More info → Run anyway**.
3. Move `physq.exe` somewhere on your `PATH`, or run it directly from the
   extracted folder in a terminal.

### Building from source instead

See [Build & run](#build--run) below — needs a Rust toolchain (`cargo build
--release`), no separate install step.

## Platform support

Prebuilt releases: macOS (Apple Silicon), Windows (x86_64), Linux (x86_64 /
aarch64). No Intel Mac build — the `ort` crate ships no prebuilt ONNX Runtime
binary for `x86_64-apple-darwin`; building ONNX Runtime from source would be
required and isn't done here.

**Linux requires glibc ≥ 2.38** (Ubuntu 24.04+, Debian 13+, Fedora 39+, …).
The prebuilt ONNX Runtime `ort` downloads calls ISO C23 libc symbols
(`__isoc23_strtol` and friends) only present from glibc 2.38 onward; older
distros (Ubuntu 22.04, Debian 12, RHEL 9, …) can't link or run the binary.

## Build & run

```sh
cargo build --release          # single binary at target/release/physq

cargo run                      # interactive TUI
cargo run -- search "電磁誘導"  # one-shot, hybrid ranking
cargo run -- search "電磁誘導" --bm25-only   # no model download, lexical only
cargo run -- search "電磁誘導" --plain | head # TSV: rank⇥score⇥id⇥question
cargo run -- cache path
cargo run -- cache clean       # drop data + index (keeps the model)
cargo run -- cache clean --all # also drop the downloaded model
```

The first run downloads the corpus (~7 MB) and — unless `--bm25-only` — the
e5 embedding model (one time, ~470 MB; cached forever). Everything works
offline afterwards. `--offline` never touches the network: it uses cached
data without an update check, and if the model was never downloaded it warns
and serves BM25-only results instead of fetching it.

### TUI keys

| Key | Action |
| --- | --- |
| type | instant BM25 ranking per keystroke |
| `Enter` (or 0.5 s pause) | semantic ranking + RRF fusion |
| `↑` `↓` / `Ctrl-P` `Ctrl-N` | select result |
| `PgUp` `PgDn` | scroll the detail pane |
| `Esc` | clear query, then quit |
| `Ctrl-C` / `Ctrl-Q` | quit |

## Cache layout

```text
<OS cache dir>/physics-notes/     # macOS: ~/Library/Caches/physics-notes
├── model/                        # fastembed-managed; downloaded once
├── data/
│   ├── version.json              # upstream manifest (when it exists)
│   ├── meta.json                 # local ETag / manifest-hash bookkeeping
│   ├── q_and_a_data.json
│   └── embeddings.json
└── index/
    └── bm25_index.bin            # bincode; tagged with tokenizer + data hash
```

Override with `--cache-dir <DIR>` or `PHYSQ_CACHE_DIR`.

## Data host

Files come from
`https://raw.githubusercontent.com/legrs/physics_notes/refs/heads/master/`
(override with `--base-url` or `PHYSQ_BASE_URL`).

**TODO:** `version.json` is being added to the upstream pipeline. Until it
lands, physq falls back to conditional (ETag) fetches of the data files and
prints a one-line warning. Once the pipeline emits `version.json` (hash per
file + `tokenizer` + `embedding_model`), startup fetches only changed files
and the warning disappears — no physq change needed.

## Semantic search

Enabled by default. `physq search` and the TUI load
`multilingual-e5-small` (384-dim, matching `embeddings.json["small"]`) via
fastembed; the model is cached under `model/`. Use `--model large` to switch
to `multilingual-e5-large` + `embeddings.json["large"]` (1024-dim, slower,
bigger download). Use `--bm25-only` to skip the semantic stage entirely.

If the model can't be loaded (e.g. offline before the first download), physq
warns and serves BM25-only results. If a *shared-artifact invariant* is
broken (wrong embedding dimensions, missing matrix, ragged vectors), it fails
loudly instead — that means the local data no longer matches what the
pipeline ships and a `cache clean` / upstream fix is needed.

## Ranking parity notes

The algorithm is the web's (`search.html`), confirmed constant for constant:
BM25 k1=1.2 b=0.75 over the **expanded** query terms; +10 exact-question,
+3 search_text/question contains, +1 keyword/synonym word hits, +2 adjacent
word pairs, Levenshtein typo score (+2/+1), character-bigram score (+0.5
each); × `priority`; +0.5 to ids related from the top-3; RRF k=60 with
semantic weight 2.0 over the full embedded corpus.

One deliberate deviation (owner-approved): the query is segmented with a
real tokenizer (lindera + IPADIC, plus the same `<2-char JP token` filter
`build.js` uses and hiragana↔katakana variants) instead of the web's
CJK-bigram hack. Corpus side is untouched: BM25 tokens are exactly
`search_text.toLowerCase().split(/\s+/)` as shipped by the pipeline.

## Architecture (UI is swappable)

All ranking/data logic is UI-agnostic and lives behind `src/engine.rs`:
`Engine` (fetch → corpus → BM25 index → query tokenizer, plus the full BM25
stage), `SemanticEngine` (query embedder + pre-computed matrix), and
`hybrid()` (RRF). The frontends — `tui/` (ratatui), `spinner.rs`, and the
printing half of `cli.rs` — are thin consumers. Polishing or replacing the
UI must not touch anything outside those three files.

```text
query.rs   bm25/   semantic/   rank/   data/   model.rs     ← pure logic
                └────── engine.rs (facade) ──────┘
        cli.rs (one-shot printing)   tui/ + spinner.rs (UI)  ← swappable
```

## Tests

```sh
cargo test        # unit tests + real-data sanity (uses ../q_and_a_data.json)
cargo clippy
cargo fmt
node scripts/parity_check.js   # runs the REAL search.html functions on the
                               # same vectors the Rust tests assert — fails
                               # if the web algorithm ever drifts
```
