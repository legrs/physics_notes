# physq — Physics Notes terminal search

A Rust TUI that searches the Physics Notes Q&A corpus **locally**, with the
web version's hybrid ranking: BM25 (with all field boosts) + multilingual-e5
semantic similarity, fused with RRF (k=60, semantic ×2.0).

The web's data artifacts are the source of truth: `physq` **fetches**
`q_and_a_data.json` and the pre-computed `embeddings.json` and never
recomputes corpus embeddings or re-tokenizes the corpus. Only the *query*
embedding is computed at runtime (fastembed / ONNX, no Python).

## Requirements & installation

**[Latest release](https://github.com/legrs/physics_notes/releases/)** — grab the
binary for your platform below; `physq` is a single, self-contained executable (no
runtime dependencies to install separately; the ~470 MB embedding model downloads on
first use, not at install time). The commands below always fetch whatever the current
latest release is, so they never need updating for a new version — and once installed,
`physq update` (see [Updating](#updating)) keeps it that way without re-running them.

> The Releases page may also list `-rcN` (release-candidate) builds for early testing
> of upcoming features. They're marked as pre-releases and never become "latest" until
> promoted to a real release — stick to the latest release above unless you
> specifically want to try one (`physq update --beta` opts into them).

| Platform | Requirement | Binary |
| --- | --- | --- |
| macOS | Apple Silicon (M1+); no Intel build (see below) | `physq-bin-aarch64-apple-darwin` |
| Windows | x86_64 | `physq-bin-x86_64-pc-windows-msvc.exe` |
| Linux | x86_64 or aarch64; **glibc ≥ 2.38** (Ubuntu 24.04+, Debian 13+, Fedora 39+, …) | `physq-bin-{x86_64,aarch64}-unknown-linux-gnu` |

These are unarchived binaries — nothing to extract.

### macOS

```sh
curl -Lo physq https://github.com/legrs/physics_notes/releases/latest/download/physq-bin-aarch64-apple-darwin
chmod +x physq
# unsigned binary: macOS Gatekeeper will refuse to run it until you clear the
# quarantine flag it sets on anything downloaded from a browser/curl
xattr -d com.apple.quarantine physq
sudo mv physq /usr/local/bin/   # or anywhere on your PATH
physq --version
```

### Linux

```sh
# swap x86_64 for aarch64 if you're on an arm64 machine
curl -Lo physq https://github.com/legrs/physics_notes/releases/latest/download/physq-bin-x86_64-unknown-linux-gnu
chmod +x physq
sudo mv physq /usr/local/bin/   # or anywhere on your PATH
physq --version
```

### Windows

1. Download
   [physq-bin-x86_64-pc-windows-msvc.exe](https://github.com/legrs/physics_notes/releases/latest/download/physq-bin-x86_64-pc-windows-msvc.exe)
   and rename it to `physq.exe`.
2. Running it may trigger SmartScreen ("Windows protected your PC") since the binary
   is unsigned — click **More info → Run anyway**.
3. Move `physq.exe` somewhere on your `PATH`, or run it directly from the folder you
   downloaded it to in a terminal.

### Building from source instead

See [Build & run](#build--run) below — needs a Rust toolchain (`cargo build
--release`), no separate install step.

### Updating

```sh
physq update              # latest stable release
physq update --check      # just report what's available, don't install
physq update --beta       # include release-candidate (rc) builds
```

Downloads the matching binary straight from GitHub Releases, verifies its
SHA-256 against the release's `checksums.txt`, and replaces the running
executable in place (no re-running the install steps above, no re-clearing
quarantine/SmartScreen — those are only ever attached by a browser/curl
download, and `physq update` fetches the file itself). Requires network
access (fails clearly under `--offline`).

Channels compare by SemVer, not recency, so switching **from** `--beta`
**to** the stable channel never silently downgrades you: a plain
`physq update` refuses (with a clear message) if the currently running
version is newer than every published stable release — e.g. running
`0.2.0-rc1` when `0.1.1` is still the latest stable tag. Pass `--force` to
install the resolved version anyway.

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
cargo run -- --bm25-only       # interactive TUI, BM25-only (no model download)
cargo run -- search "電磁誘導"  # one-shot, hybrid ranking
cargo run -- search "電磁誘導" --bm25-only   # no model download, lexical only
cargo run -- search "電磁誘導" --model none  # same thing, spelled via --model
cargo run -- search "電磁誘導" --plain | head # TSV: rank⇥score⇥id⇥question
cargo run -- cache path
cargo run -- cache clean             # drop data + index (keeps the model)
cargo run -- cache clean --all       # also drop the downloaded model
cargo run -- cache clean --model-only # drop only the model (keeps data + index)
cargo run -- update --check    # check for a newer release (see Updating below)
```

The first run downloads the corpus (~7 MB) and — unless `--bm25-only` /
`--model none` — the e5 embedding model (one time, ~470 MB; cached forever).
Everything works offline afterwards. `--bm25-only` and `--model none` are
global flags: they apply to both the interactive TUI and `search` alike, and
skip the semantic stage (and its model download) entirely. `--offline` never
touches the network: it uses cached data without an update check, and if the
model was never downloaded it warns and serves BM25-only results instead of
fetching it.

### TUI keys

| Key | Action |
| --- | --- |
| type | instant BM25 ranking per keystroke |
| `Enter` (or 0.5 s pause) | semantic ranking + RRF fusion — or run a `/command` |
| `↑` `↓` / `Ctrl-P` `Ctrl-N` | select result (auto-scrolls into view) |
| `PgUp` `PgDn` | scroll the detail pane |
| `Tab` | browse the selected item's Related list; `↑` `↓` pick, `Enter` jumps |
| `Esc` | close a `/help`/`/config` screen, then exit Related-browsing, then clear query, then quit |
| `Ctrl-C` / `Ctrl-Q` | quit |

Mouse: wheel over Results scrolls the list (selection is untouched — arrow
keys still auto-scroll back to it); wheel over Detail scrolls the text;
clicking a result selects it; clicking a Related entry in Detail jumps to it.
"Jumping" to a Related item re-searches its question, so it reuses the normal
ranking pipeline instead of a separate pinned-detail mode.

### Slash commands

Typed into the same input box, run on `Enter`:

| Command | Effect |
| --- | --- |
| `/semantic small` / `/semantic large` | switch the embedding model at runtime (reloads, may download on first use) |
| `/semantic max` | ensemble mode: rank with both e5 models and RRF-fuse each list with BM25 (most accurate; loads both models) |
| `/semantic none` | turn semantic off at runtime — BM25-only until switched back |
| `/config` | interactive settings screen — `↑` `↓` picks a field, `←` `→`/`Enter` changes it (model size cycles small → large → max → none, offline mode), plus read-only info (base URL, cache dir, tokenizer) |
| `/help` | shortcut reference (keyboard, mouse, commands) |
| `/exit` (or `/quit`) | quit |

## Cache layout

```text
<OS cache dir>/physics-notes/     # macOS: ~/Library/Caches/physics-notes
├── model/                        # fastembed-managed; downloaded once
├── data/
│   ├── version.json              # upstream manifest (hash/size per file + tokenizer/embedding_model tags)
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

Startup fetches the small `version.json` manifest first (hash + size per
file, plus `tokenizer`/`embedding_model` tags) and only re-downloads
`q_and_a_data.json`/`embeddings.json` when their hash actually changed — the
pipeline (`scripts/build.js`) regenerates `version.json` on every run, so
it's always there. If the data host is ever unreachable for `version.json`
specifically but a local cache exists, physq falls back to conditional
(ETag) fetches of the data files directly and warns once.

## Semantic search

Enabled by default. `physq search` and the TUI load
`multilingual-e5-small` (384-dim, matching `embeddings.json["small"]`) via
fastembed; the model is cached under `model/`. Use `--model large` to switch
to `multilingual-e5-large` + `embeddings.json["large"]` (1024-dim, slower,
bigger download). Use `--model max` for the ensemble: it ranks with **both**
e5 models and RRF-fuses each list alongside BM25, so a hit both models place
2nd–3rd can outrank one a single model puts 1st — most accurate, slowest,
loads both models (query embedded once per model). Use `--bm25-only` (or
`--model none`) to skip the semantic stage entirely — no model download,
lexical ranking only, in both the TUI and `search`. Free the downloaded
model without touching cached data with `cache clean --model-only`.

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
                └────── engine.rs (facade) ──────┘          update.rs (self-update, CLI-only)
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
