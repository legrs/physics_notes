//! physq — local hybrid search (BM25 + e5 semantic, RRF-fused) for the
//! Physics Notes Q&A corpus.
//!
//! The web version's data artifacts (`q_and_a_data.json`, `embeddings.json`)
//! are the source of truth. This crate fetches them and never recomputes
//! corpus embeddings or re-tokenizes the corpus (see CLAUDE.md §2).

pub mod bm25;
pub mod cli;
pub mod config;
pub mod data;
pub mod engine;
pub mod model;
pub mod query;
pub mod rank;
mod real_data_tests;
pub mod semantic;
pub mod spinner;
pub mod tui;
