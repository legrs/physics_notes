//! Interactive TUI (ratatui + crossterm, CLAUDE.md §3, §11).
//!
//! 2-tier UX (§6): every keystroke re-runs BM25 instantly; submit (Enter) or
//! a short typing debounce triggers the semantic ranking, RRF-fused when it
//! arrives. All heavy work (fetch, model load, query embedding) runs on
//! background tasks; the render loop only animates the spinner (§11).
//!
//! Beyond the core search loop, the UI supports: mouse wheel scrolling and
//! click-to-select on both panes, jumping to a `related[]` item (by
//! re-searching its question — see `jump_to_related`), slash commands typed
//! into the input box (`/semantic small|large|max|none`, `/config`, `/help`,
//! `/exit`), and a `Tab`-focused keyboard path through Related so nothing
//! here requires a mouse. `/semantic none` (or launching with `--model none`
//! / `--bm25-only`) disables the semantic stage entirely: no model download,
//! BM25-only results.

mod command;

use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use ratatui::Frame;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, CustomWeights, ModelSel, ModelSize};
use crate::engine::{Engine, SemanticEngine, hybrid, hybrid_custom};
use crate::query::prepare_query;
use crate::semantic::SemanticError;
use crate::spinner;

use command::{ParsedCommand, parse_command};

const SEMANTIC_DEBOUNCE: Duration = Duration::from_millis(500);
const DETAIL_SCROLL_STEP: u16 = 5;
const MOUSE_SCROLL_STEP: u16 = 3;

/// Per-model descriptions, shown under `/config`'s Semantic status and on the
/// `/semantic` command-suggestion screen. Order matches the `/config` cycle.
const MODEL_DESCRIPTIONS: &[(&str, &str)] = &[
    ("small", "e5-small · 384d — fast, lower memory"),
    ("large", "e5-large · 1024d — slower, more accurate"),
    (
        "max",
        "ensemble of small + large, RRF-fused — most accurate, loads both",
    ),
    ("none", "semantic off — BM25-only, no model download"),
];

/// Status-bar marquee cadence: milliseconds per one-character scroll step.
const MARQUEE_STEP_MS: u128 = 200;
/// Gap rendered between marquee loop repetitions.
const MARQUEE_GAP: &str = "      ";

enum AppMsg {
    Progress(String),
    Data(Box<Result<Engine, String>>),
    SemanticUp {
        generation: u64,
    },
    SemanticDown {
        generation: u64,
        error: String,
        invariant: bool,
    },
    SemanticRanked {
        generation: u64,
        seq: u64,
        /// One ranked list per configured model (1 for a single model, 2 for
        /// the `max` ensemble); the UI thread RRF-fuses them with BM25.
        ranked: Vec<Vec<(u32, f64)>>,
    },
    SemanticQueryFailed {
        generation: u64,
        seq: u64,
        error: String,
        invariant: bool,
    },
}

struct SemReq {
    seq: u64,
    q: String,
}

#[derive(PartialEq)]
enum SemState {
    /// Data not loaded yet, worker not started.
    Off,
    Init,
    Ready,
    /// User (or a launch flag) turned semantic off; no worker running by
    /// choice, not by failure. Distinct from `Off`, which is just the
    /// transient pre-data-load state.
    Disabled,
    Failed {
        invariant: bool,
    },
}

#[derive(Clone, Copy, PartialEq)]
enum ResultsMode {
    Bm25,
    Hybrid,
}

/// Which pane keyboard navigation controls. `Tab` switches into `Related`
/// only when the selected record has related items; anything that isn't a
/// Related-focus key (typing, Esc, …) switches back to `Results`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PaneFocus {
    Results,
    Related,
}

/// A full-body overlay that replaces Results+Detail. `Help` is a static
/// reference; `Config` is a small interactive settings form.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Overlay {
    None,
    Help,
    Config,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfigField {
    Model,
    Offline,
    /// `--debug` custom-mode weight rows; only present when the model is
    /// `custom`. Adjusted with ←/→ (see `App::config_fields`).
    WeightBm25,
    WeightSmall,
    WeightLarge,
}

struct App {
    cfg: Config,
    input: String,
    /// Cursor as a char offset into `input`.
    cursor: usize,
    data: Option<Engine>,
    data_error: Option<String>,
    sem_state: SemState,
    sem_error: Option<String>,
    sem_tx: Option<std_mpsc::Sender<SemReq>>,
    /// Handoff for a freshly-created request channel; `run_loop` picks this
    /// up and spawns a worker for it, whether from the initial load or a
    /// later `/semantic` switch.
    pending_sem_rx: Option<std_mpsc::Receiver<SemReq>>,
    /// Bumped every time a new semantic worker is spawned so stale messages
    /// from an abandoned (switched-away-from) worker can be dropped.
    sem_generation: u64,
    /// Latest fully-boosted BM25 list (what RRF consumes).
    bm25_results: Vec<(u32, f64)>,
    /// What the list shows (BM25 or RRF-merged).
    results: Vec<(u32, f64)>,
    results_mode: ResultsMode,
    selected: Option<usize>,
    /// Absolute row offset into the rendered Results text, independent of
    /// `selected` (mouse wheel moves this without touching selection).
    results_scroll: u16,
    /// True after a selection change (arrow/click/new query): the next draw
    /// clamps `results_scroll` to keep the selection visible. False after a
    /// wheel scroll, which is free to move away from the selection.
    scroll_follow_selection: bool,
    /// row → result index, rebuilt every draw; consumed by mouse clicks.
    results_row_targets: Vec<usize>,
    list_area: Rect,
    detail_scroll: u16,
    /// row → corpus index of a clickable Related entry, rebuilt every draw.
    detail_row_targets: Vec<Option<u32>>,
    detail_area: Rect,
    pane_focus: PaneFocus,
    related_selected: Option<usize>,
    overlay: Overlay,
    config_cursor: usize,
    /// Vertical scroll offset (rows) for the `/config` overlay body.
    config_scroll: u16,
    command_error: Option<String>,
    /// Bumped on every input change; stale semantic responses are dropped.
    seq: u64,
    last_requested_seq: u64,
    last_input_at: Instant,
    force_semantic: bool,
    /// (seq, started) of the in-flight semantic request, for the spinner.
    pending_sem: Option<(u64, Instant)>,
    /// Current phase label + start, for slow steps (fetch, model load).
    phase: Option<(String, Instant)>,
    warnings: Vec<String>,
    q_lower: String,
    should_quit: bool,
    /// Fixed epoch used to drive time-based UI animation (the status-bar
    /// marquee). Set once at construction.
    app_start: Instant,
    /// Snapshot of the last real (non-command) hybrid query, kept alive so the
    /// `--debug` custom weight editor can re-fuse it and show a live preview
    /// even after typing `/config` cleared the visible results.
    snap_query: String,
    snap_bm25: Vec<(u32, f64)>,
    snap_semantic: Vec<Vec<(u32, f64)>>,
}

impl App {
    fn new(cfg: Config) -> Self {
        Self {
            cfg,
            input: String::new(),
            cursor: 0,
            data: None,
            data_error: None,
            sem_state: SemState::Off,
            sem_error: None,
            sem_tx: None,
            pending_sem_rx: None,
            sem_generation: 0,
            bm25_results: Vec::new(),
            results: Vec::new(),
            results_mode: ResultsMode::Bm25,
            selected: None,
            results_scroll: 0,
            scroll_follow_selection: true,
            results_row_targets: Vec::new(),
            list_area: Rect::default(),
            detail_scroll: 0,
            detail_row_targets: Vec::new(),
            detail_area: Rect::default(),
            pane_focus: PaneFocus::Results,
            related_selected: None,
            overlay: Overlay::None,
            config_cursor: 0,
            config_scroll: 0,
            command_error: None,
            seq: 0,
            last_requested_seq: 0,
            last_input_at: Instant::now(),
            force_semantic: false,
            pending_sem: None,
            phase: Some(("Starting…".to_string(), Instant::now())),
            warnings: Vec::new(),
            q_lower: String::new(),
            should_quit: false,
            app_start: Instant::now(),
            snap_query: String::new(),
            snap_bm25: Vec::new(),
            snap_semantic: Vec::new(),
        }
    }

    /// The config-screen fields in visual/navigation order. The custom-mode
    /// weight rows appear only under `--debug` when the model is `custom`.
    fn config_fields(&self) -> Vec<ConfigField> {
        let mut f = vec![ConfigField::Model, ConfigField::Offline];
        if self.cfg.debug && self.cfg.model == ModelSel::Custom {
            f.push(ConfigField::WeightBm25);
            f.push(ConfigField::WeightSmall);
            f.push(ConfigField::WeightLarge);
        }
        f
    }

    /// The currently focused config field (cursor clamped to the field list).
    fn focused_config_field(&self) -> ConfigField {
        let fields = self.config_fields();
        fields[self.config_cursor.min(fields.len() - 1)]
    }

    fn selected_doc(&self) -> Option<u32> {
        self.selected
            .and_then(|i| self.results.get(i))
            .map(|&(d, _)| d)
    }

    fn is_command_input(&self) -> bool {
        self.input.trim_start().starts_with('/')
    }

    fn current_related_count(&self) -> usize {
        let Some(data) = &self.data else { return 0 };
        let Some(doc) = self.selected_doc() else {
            return 0;
        };
        data.corpus.records[doc as usize].related.len()
    }

    fn refresh_bm25(&mut self) {
        self.seq += 1;
        self.last_input_at = Instant::now();
        self.pending_sem = None;
        self.detail_scroll = 0;
        self.results_scroll = 0;
        self.scroll_follow_selection = true;
        self.pane_focus = PaneFocus::Results;
        self.related_selected = None;
        self.command_error = None;
        self.q_lower = prepare_query(&self.input);

        let Some(data) = &self.data else {
            self.bm25_results.clear();
            self.results.clear();
            self.selected = None;
            return;
        };
        if self.is_command_input() || self.q_lower.is_empty() {
            self.bm25_results.clear();
            self.results.clear();
            self.selected = None;
            self.results_mode = ResultsMode::Bm25;
            return;
        }
        self.bm25_results = data.bm25(&self.q_lower);
        self.results = self.bm25_results.clone();
        self.results_mode = ResultsMode::Bm25;
        self.selected = if self.results.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    fn maybe_request_semantic(&mut self) {
        if self.sem_state != SemState::Ready || self.q_lower.is_empty() || self.is_command_input() {
            return;
        }
        if self.seq <= self.last_requested_seq {
            return;
        }
        if !self.force_semantic && self.last_input_at.elapsed() < SEMANTIC_DEBOUNCE {
            return;
        }
        if let Some(tx) = &self.sem_tx
            && tx
                .send(SemReq {
                    seq: self.seq,
                    q: self.q_lower.clone(),
                })
                .is_ok()
        {
            self.last_requested_seq = self.seq;
            self.pending_sem = Some((self.seq, Instant::now()));
        }
        self.force_semantic = false;
    }

    fn handle_msg(&mut self, msg: AppMsg) {
        match msg {
            AppMsg::Progress(label) => {
                let started = self
                    .phase
                    .take()
                    .map(|(_, t)| t)
                    .unwrap_or_else(Instant::now);
                self.phase = Some((label, started));
            }
            AppMsg::Data(result) => match *result {
                Ok(bundle) => {
                    self.warnings.extend(bundle.warnings.iter().cloned());
                    self.data = Some(bundle);
                    if self.cfg.model.is_enabled() {
                        self.sem_state = SemState::Init;
                        self.phase = Some(("Loading semantic model…".to_string(), Instant::now()));
                        let (tx, rx) = std_mpsc::channel();
                        self.sem_tx = Some(tx);
                        self.pending_sem_rx = Some(rx);
                    } else {
                        self.sem_state = SemState::Disabled;
                        self.phase = None;
                    }
                    self.refresh_bm25();
                }
                Err(e) => {
                    self.phase = None;
                    self.data_error = Some(e);
                }
            },
            AppMsg::SemanticUp { generation } => {
                if generation != self.sem_generation {
                    return;
                }
                self.sem_state = SemState::Ready;
                self.phase = None;
            }
            AppMsg::SemanticDown {
                generation,
                error,
                invariant,
            } => {
                if generation != self.sem_generation {
                    return;
                }
                self.sem_state = SemState::Failed { invariant };
                self.sem_error = Some(error);
                self.phase = None;
            }
            AppMsg::SemanticRanked {
                generation,
                seq,
                ranked,
            } => {
                if generation != self.sem_generation {
                    return;
                }
                if let Some((pseq, _)) = self.pending_sem
                    && seq >= pseq
                {
                    self.pending_sem = None;
                }
                if seq == self.seq {
                    self.results = if self.cfg.model == ModelSel::Custom {
                        hybrid_custom(&self.bm25_results, &ranked, &self.cfg.weights)
                    } else {
                        hybrid(&self.bm25_results, &ranked)
                    };
                    self.results_mode = ResultsMode::Hybrid;
                    self.selected = if self.results.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                    self.detail_scroll = 0;
                    self.results_scroll = 0;
                    self.scroll_follow_selection = true;
                    // Snapshot this query so the /config weight editor can
                    // re-fuse it live even after the visible results are gone.
                    self.snap_query = self.q_lower.clone();
                    self.snap_bm25 = self.bm25_results.clone();
                    self.snap_semantic = ranked;
                }
            }
            AppMsg::SemanticQueryFailed {
                generation,
                seq,
                error,
                invariant,
            } => {
                if generation != self.sem_generation {
                    return;
                }
                if let Some((pseq, _)) = self.pending_sem
                    && seq >= pseq
                {
                    self.pending_sem = None;
                }
                self.sem_error = Some(error);
                if invariant {
                    self.sem_state = SemState::Failed { invariant: true };
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Windows terminals emit Release events too; act only on Press/Repeat.
        if key.kind == KeyEventKind::Release {
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        if matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q')) && ctrl {
            self.should_quit = true;
            return;
        }

        if self.overlay != Overlay::None {
            self.handle_overlay_key(key);
            return;
        }

        if self.pane_focus == PaneFocus::Related && self.handle_related_focus_key(key) {
            return;
        }

        match key.code {
            KeyCode::Esc => {
                if !self.input.is_empty() {
                    self.input.clear();
                    self.cursor = 0;
                    self.refresh_bm25();
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Enter => self.submit_or_run_command(),
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let idx = byte_index(&self.input, self.cursor - 1);
                    self.input.remove(idx);
                    self.cursor -= 1;
                    self.refresh_bm25();
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.input.chars().count() {
                    let idx = byte_index(&self.input, self.cursor);
                    self.input.remove(idx);
                    self.refresh_bm25();
                }
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.input.chars().count()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.input.chars().count(),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::Char('p') if ctrl => self.move_selection(-1),
            KeyCode::Char('n') if ctrl => self.move_selection(1),
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(DETAIL_SCROLL_STEP)
            }
            KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_add(DETAIL_SCROLL_STEP)
            }
            KeyCode::Tab => self.toggle_related_focus(),
            KeyCode::Char(c) if !ctrl && !c.is_control() => {
                let idx = byte_index(&self.input, self.cursor);
                self.input.insert(idx, c);
                self.cursor += 1;
                self.refresh_bm25();
            }
            _ => {}
        }
    }

    /// Handles keys while a `Help`/`Config` overlay is showing. `Config`
    /// intercepts navigation/toggle keys; everything else (including every
    /// key in `Help`) closes the overlay and re-dispatches through the
    /// normal path, so typing immediately continues as a new search.
    fn handle_overlay_key(&mut self, key: KeyEvent) {
        if self.overlay == Overlay::Config {
            let n = self.config_fields().len();
            match key.code {
                KeyCode::Esc => {
                    self.overlay = Overlay::None;
                    return;
                }
                KeyCode::Up => {
                    self.config_cursor = (self.config_cursor + n - 1) % n;
                    return;
                }
                KeyCode::Down => {
                    self.config_cursor = (self.config_cursor + 1) % n;
                    return;
                }
                KeyCode::PageUp => {
                    self.config_scroll = self.config_scroll.saturating_sub(DETAIL_SCROLL_STEP);
                    return;
                }
                KeyCode::PageDown => {
                    self.config_scroll = self.config_scroll.saturating_add(DETAIL_SCROLL_STEP);
                    return;
                }
                KeyCode::Left => {
                    self.adjust_config_field(-1.0);
                    return;
                }
                KeyCode::Right => {
                    self.adjust_config_field(1.0);
                    return;
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.activate_config_field();
                    return;
                }
                _ => {}
            }
        } else if key.code == KeyCode::Esc {
            self.overlay = Overlay::None;
            return;
        }

        self.overlay = Overlay::None;
        self.handle_key(key);
    }

    /// Enter/Space on the focused config field: Model/Offline cycle/toggle
    /// forward; a weight row resets to its default.
    fn activate_config_field(&mut self) {
        match self.focused_config_field() {
            ConfigField::Model => self.cycle_model(true),
            ConfigField::Offline => self.toggle_offline(),
            ConfigField::WeightBm25 => {
                self.cfg.weights.bm25 = CustomWeights::default().bm25;
            }
            ConfigField::WeightSmall => {
                self.cfg.weights.small = CustomWeights::default().small;
            }
            ConfigField::WeightLarge => {
                self.cfg.weights.large = CustomWeights::default().large;
            }
        }
    }

    /// ←/→ on the focused config field: Model/Offline step through their
    /// options; a weight row nudges its coefficient by ±STEP (live).
    fn adjust_config_field(&mut self, dir: f64) {
        match self.focused_config_field() {
            ConfigField::Model => self.cycle_model(dir > 0.0),
            ConfigField::Offline => self.toggle_offline(),
            ConfigField::WeightBm25 => bump_weight(&mut self.cfg.weights.bm25, dir),
            ConfigField::WeightSmall => bump_weight(&mut self.cfg.weights.small, dir),
            ConfigField::WeightLarge => bump_weight(&mut self.cfg.weights.large, dir),
        }
    }

    /// Cycle the semantic model. Order is small → large → max → none, with
    /// `custom` inserted before `none` only under `--debug`.
    fn cycle_model(&mut self, forward: bool) {
        let order: &[ModelSel] = if self.cfg.debug {
            &[
                ModelSel::Single(ModelSize::Small),
                ModelSel::Single(ModelSize::Large),
                ModelSel::Max,
                ModelSel::Custom,
                ModelSel::Off,
            ]
        } else {
            &[
                ModelSel::Single(ModelSize::Small),
                ModelSel::Single(ModelSize::Large),
                ModelSel::Max,
                ModelSel::Off,
            ]
        };
        let cur = order.iter().position(|&m| m == self.cfg.model).unwrap_or(0);
        let next = if forward {
            (cur + 1) % order.len()
        } else {
            (cur + order.len() - 1) % order.len()
        };
        self.reload_semantic(order[next]);
    }

    fn toggle_offline(&mut self) {
        self.cfg.offline = !self.cfg.offline;
        // Convenience retry: if we'd previously failed only because we were
        // offline, going back online should just work again.
        if !self.cfg.offline && matches!(self.sem_state, SemState::Failed { invariant: false }) {
            self.reload_semantic(self.cfg.model);
        }
    }

    /// Fuse the last-query snapshot with the current custom weights — the
    /// source for the `/config` live preview.
    fn custom_preview(&self) -> Vec<(u32, f64)> {
        hybrid_custom(&self.snap_bm25, &self.snap_semantic, &self.cfg.weights)
    }

    fn submit(&mut self) {
        // Submit: semantic + RRF (fires immediately, or as soon as the model
        // is ready). Re-arm even if this seq was already requested (e.g. the
        // response was stale or lost).
        self.force_semantic = true;
        self.last_requested_seq = self.seq.saturating_sub(1).min(self.last_requested_seq);
    }

    fn submit_or_run_command(&mut self) {
        let Some(cmd) = parse_command(&self.input) else {
            self.submit();
            return;
        };
        self.input.clear();
        self.cursor = 0;
        self.command_error = None;
        match cmd {
            ParsedCommand::Exit => self.should_quit = true,
            ParsedCommand::Help => self.overlay = Overlay::Help,
            ParsedCommand::Config => {
                self.overlay = Overlay::Config;
                self.config_cursor = 0;
                self.config_scroll = 0;
            }
            ParsedCommand::Semantic(size) => self.reload_semantic(size),
            ParsedCommand::Unknown(s) => {
                self.command_error = Some(format!("unknown command: {s} (try /help)"));
            }
        }
        self.refresh_bm25();
    }

    fn move_selection(&mut self, delta: i64) {
        if self.results.is_empty() {
            return;
        }
        let cur = self.selected.unwrap_or(0) as i64;
        let next = (cur + delta).clamp(0, self.results.len() as i64 - 1) as usize;
        self.selected = Some(next);
        self.detail_scroll = 0;
        self.scroll_follow_selection = true;
    }

    fn toggle_related_focus(&mut self) {
        match self.pane_focus {
            PaneFocus::Results => {
                if self.current_related_count() > 0 {
                    self.pane_focus = PaneFocus::Related;
                    self.related_selected = Some(0);
                }
            }
            PaneFocus::Related => {
                self.pane_focus = PaneFocus::Results;
                self.related_selected = None;
            }
        }
    }

    /// Returns `true` if the key was consumed. `false` means "not a
    /// related-focus key" — the caller falls back to `Results` focus and
    /// normal key handling (e.g. typing to search).
    fn handle_related_focus_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc | KeyCode::Tab => {
                self.pane_focus = PaneFocus::Results;
                self.related_selected = None;
                true
            }
            KeyCode::Up => {
                self.move_related_selection(-1);
                true
            }
            KeyCode::Down => {
                self.move_related_selection(1);
                true
            }
            KeyCode::Enter => {
                self.activate_related_selection();
                true
            }
            _ => {
                self.pane_focus = PaneFocus::Results;
                self.related_selected = None;
                false
            }
        }
    }

    fn move_related_selection(&mut self, delta: i64) {
        let len = self.current_related_count();
        if len == 0 {
            self.pane_focus = PaneFocus::Results;
            self.related_selected = None;
            return;
        }
        let cur = self.related_selected.unwrap_or(0) as i64;
        let next = (cur + delta).rem_euclid(len as i64) as usize;
        self.related_selected = Some(next);
    }

    fn activate_related_selection(&mut self) {
        let Some(i) = self.related_selected else {
            return;
        };
        let id = {
            let Some(data) = &self.data else { return };
            let Some(doc) = self.selected_doc() else {
                return;
            };
            let Some(id) = data.corpus.records[doc as usize].related.get(i).cloned() else {
                return;
            };
            id
        };
        self.jump_to_related(&id);
    }

    /// "Jump" to a related item by re-searching its first question — BM25's
    /// exact-question-match boost (+10, CLAUDE.md §6) reliably puts it at
    /// rank 0, so this reuses the whole existing search pipeline instead of
    /// needing a separate "pinned detail" state.
    fn jump_to_related(&mut self, target_id: &str) {
        let q = {
            let Some(data) = &self.data else { return };
            let Some(record) = data.corpus.records.iter().find(|r| r.id == target_id) else {
                return;
            };
            let Some(q) = record.questions.first().cloned() else {
                return;
            };
            q
        };
        self.input = q;
        self.cursor = self.input.chars().count();
        self.refresh_bm25();
        self.submit();
    }

    /// Switch the active semantic model at runtime, or turn it off entirely
    /// (`None`). Drops the old request sender (its worker thread's `recv()`
    /// then errors out and the thread exits) and, when switching to a model,
    /// hands `run_loop` a fresh channel to spawn a new worker for — tagged
    /// with a bumped generation so stale messages from the old worker are
    /// ignored (`handle_msg`'s `generation` checks).
    fn reload_semantic(&mut self, model: ModelSel) {
        self.cfg.model = model;
        self.sem_generation += 1;
        self.sem_error = None;
        if model.is_enabled() {
            let (tx, rx) = std_mpsc::channel();
            self.sem_tx = Some(tx);
            self.pending_sem_rx = Some(rx);
            self.sem_state = SemState::Init;
            self.phase = Some((
                format!("Switching to {} model…", model.label()),
                Instant::now(),
            ));
        } else {
            self.sem_tx = None;
            self.pending_sem_rx = None;
            self.sem_state = SemState::Disabled;
            self.phase = None;
            // Drop back to the BM25-only view immediately instead of
            // leaving a stale hybrid result set on screen.
            self.results = self.bm25_results.clone();
            self.results_mode = ResultsMode::Bm25;
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) {
        if self.overlay == Overlay::Config {
            // Config scrolls with the wheel; clicks are ignored (keyboard-driven).
            match m.kind {
                MouseEventKind::ScrollUp => {
                    self.config_scroll = self.config_scroll.saturating_sub(MOUSE_SCROLL_STEP);
                }
                MouseEventKind::ScrollDown => {
                    self.config_scroll = self.config_scroll.saturating_add(MOUSE_SCROLL_STEP);
                }
                _ => {}
            }
            return;
        }
        if self.overlay != Overlay::None {
            return; // Help is keyboard-only by design.
        }
        let in_list = rect_contains(self.list_area, m.column, m.row);
        let in_detail = rect_contains(self.detail_area, m.column, m.row);

        match m.kind {
            MouseEventKind::ScrollUp => {
                if in_list {
                    self.results_scroll = self.results_scroll.saturating_sub(MOUSE_SCROLL_STEP);
                    self.scroll_follow_selection = false;
                } else if in_detail {
                    self.detail_scroll = self.detail_scroll.saturating_sub(MOUSE_SCROLL_STEP);
                }
            }
            MouseEventKind::ScrollDown => {
                if in_list {
                    self.results_scroll = self.results_scroll.saturating_add(MOUSE_SCROLL_STEP);
                    self.scroll_follow_selection = false;
                } else if in_detail {
                    self.detail_scroll = self.detail_scroll.saturating_add(MOUSE_SCROLL_STEP);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if in_list {
                    let local_row = m.row.saturating_sub(self.list_area.y + 1);
                    let abs_row = (self.results_scroll + local_row) as usize;
                    if let Some(&item) = self.results_row_targets.get(abs_row) {
                        self.selected = Some(item);
                        self.detail_scroll = 0;
                        self.scroll_follow_selection = true;
                    }
                } else if in_detail {
                    let local_row = m.row.saturating_sub(self.detail_area.y + 1);
                    let abs_row = (self.detail_scroll + local_row) as usize;
                    let target_id = self
                        .detail_row_targets
                        .get(abs_row)
                        .copied()
                        .flatten()
                        .and_then(|idx| {
                            self.data
                                .as_ref()
                                .and_then(|d| d.corpus.records.get(idx as usize))
                                .map(|r| r.id.clone())
                        });
                    if let Some(id) = target_id {
                        self.jump_to_related(&id);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Nudge a custom RRF weight by ±`STEP`, rounded to one decimal (avoids float
/// drift like 1.9999) and clamped to `[MIN, MAX]`.
fn bump_weight(w: &mut f64, dir: f64) {
    let next = ((*w + dir * CustomWeights::STEP) * 10.0).round() / 10.0;
    *w = next.clamp(CustomWeights::MIN, CustomWeights::MAX);
}

fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

fn byte_index(s: &str, char_offset: usize) -> usize {
    s.char_indices()
        .nth(char_offset)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Rows `line` will occupy when wrapped to `width` columns. Delegates to
/// ratatui's own (CJK-aware) wrap algorithm via `Paragraph::line_count`
/// instead of hand-rolling word-wrap — critical because Japanese text has no
/// spaces, so a naive ASCII word-wrapper would never break it at all.
/// `line_count` sits behind ratatui's `unstable-rendered-line-info` feature
/// (Cargo.toml) — it's semver-exempt upstream, but it's the documented way
/// to get wrapped-row accounting, so this is the intended use, not a hack.
fn wrapped_row_count(line: &Line<'_>, width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    (Paragraph::new(line.clone())
        .wrap(Wrap { trim: false })
        .line_count(width) as u16)
        .max(1)
}

/// The semantic worker owns the `SemanticEngine` on its own thread; the
/// model loads (and on first run, downloads) behind the spinner while BM25
/// is already usable. All ranking logic lives in `engine` — this thread is
/// only channel plumbing. `generation` tags every outgoing message so `App` can
/// ignore messages from a worker generation it has since switched away from.
fn semantic_worker(
    cfg: Config,
    engine: Engine,
    generation: u64,
    req_rx: std_mpsc::Receiver<SemReq>,
    tx: std_mpsc::Sender<AppMsg>,
) {
    let mut sem = match SemanticEngine::load(&cfg, engine.corpus.clone()) {
        Ok(sem) => sem,
        Err(e) => {
            let invariant = matches!(e, SemanticError::Invariant(_));
            let _ = tx.send(AppMsg::SemanticDown {
                generation,
                error: e.to_string(),
                invariant,
            });
            return;
        }
    };
    let _ = tx.send(AppMsg::SemanticUp { generation });

    while let Ok(mut req) = req_rx.recv() {
        // Collapse a burst of requests down to the newest one.
        while let Ok(newer) = req_rx.try_recv() {
            req = newer;
        }
        let msg = match sem.rank(&req.q) {
            Ok(ranked) => AppMsg::SemanticRanked {
                generation,
                seq: req.seq,
                ranked,
            },
            Err(e) => AppMsg::SemanticQueryFailed {
                generation,
                seq: req.seq,
                invariant: matches!(e, SemanticError::Invariant(_)),
                error: e.to_string(),
            },
        };
        if tx.send(msg).is_err() {
            return;
        }
    }
}

pub fn run(cfg: Config) -> Result<()> {
    use std::io::IsTerminal;
    if !std::io::stdout().is_terminal() {
        bail!("stdout is not a terminal; use `physq search \"<query>\" --plain` for piped output");
    }

    let runtime = tokio::runtime::Runtime::new().context("failed to start async runtime")?;
    let (tx, rx) = std_mpsc::channel::<AppMsg>();

    {
        let cfg = cfg.clone();
        let tx_task = tx.clone();
        let tx_progress = tx.clone();
        runtime.spawn(async move {
            let progress = move |s: &str| {
                let _ = tx_progress.send(AppMsg::Progress(s.to_string()));
            };
            let result = Engine::load(&cfg, &progress).await;
            let _ = tx_task.send(AppMsg::Data(Box::new(result.map_err(|e| format!("{e:#}")))));
        });
    }

    let mut app = App::new(cfg);
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    let result = run_loop(&mut terminal, &mut app, rx, tx);
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rx: std_mpsc::Receiver<AppMsg>,
    tx: std_mpsc::Sender<AppMsg>,
) -> Result<()> {
    loop {
        while let Ok(msg) = rx.try_recv() {
            app.handle_msg(msg);
        }
        // The semantic worker is (re)spawned lazily whenever a new request
        // channel shows up — on initial data load and on every `/semantic`
        // switch alike.
        if let Some(req_rx) = app.pending_sem_rx.take()
            && let Some(engine) = &app.data
        {
            let cfg = app.cfg.clone();
            let engine = engine.clone();
            let tx = tx.clone();
            let generation = app.sem_generation;
            std::thread::spawn(move || semantic_worker(cfg, engine, generation, req_rx, tx));
        }
        app.maybe_request_semantic();

        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(Duration::from_millis(spinner::FRAME_MS))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(m) => app.handle_mouse(m),
                Event::Paste(s) => {
                    for c in s.chars().filter(|c| !c.is_control()) {
                        let idx = byte_index(&app.input, app.cursor);
                        app.input.insert(idx, c);
                        app.cursor += 1;
                    }
                    app.refresh_bm25();
                }
                _ => {}
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}

fn draw(frame: &mut Frame, app: &mut App) {
    let [input_area, body, status_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    // ── input ──────────────────────────────────────────────────────────
    let input_line = Line::from(vec![
        Span::styled("» ", Style::default().fg(Color::Cyan)),
        Span::raw(app.input.as_str()),
    ]);
    frame.render_widget(
        Paragraph::new(input_line).block(Block::bordered().title(" Physics Notes ")),
        input_area,
    );
    let prefix_width = "» ".width() as u16;
    let cursor_x = {
        let byte = byte_index(&app.input, app.cursor);
        app.input[..byte].width() as u16
    };
    frame.set_cursor_position(Position::new(
        input_area.x + 1 + prefix_width + cursor_x,
        input_area.y + 1,
    ));

    if app.overlay != Overlay::None {
        draw_overlay(frame, app, body);
        render_status(frame, app, status_area);
        return;
    }

    // ── body ───────────────────────────────────────────────────────────
    let [list_area, detail_area] =
        Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).areas(body);
    app.list_area = list_area;
    app.detail_area = detail_area;

    draw_results(frame, app, list_area);
    draw_detail(frame, app, detail_area);

    // ── status ─────────────────────────────────────────────────────────
    render_status(frame, app, status_area);
}

/// Results is a hand-rolled listbox on top of one `Paragraph` (not
/// `List`/`ListState`): questions wrap naturally via ratatui's own wrap
/// algorithm (`Wrap{trim:false}`), and `wrapped_row_count` (same algorithm,
/// called per-line) gives us exact row accounting for scroll-into-view math
/// and mouse-click hit-testing, without duplicating ratatui's wrapping.
fn draw_results(frame: &mut Frame, app: &mut App, area: Rect) {
    let mode_label = match app.results_mode {
        ResultsMode::Bm25 => "BM25",
        ResultsMode::Hybrid => "Hybrid (BM25+semantic RRF)",
    };
    let title = format!(" Results · {} · {} ", mode_label, app.results.len());
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    let inner_width = inner.width.max(1);
    let inner_height = inner.height;

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut row_of_item: Vec<usize> = Vec::new();

    if let Some(data) = &app.data {
        for (i, &(doc, score)) in app.results.iter().enumerate() {
            let record = &data.corpus.records[doc as usize];
            let q = record
                .questions
                .first()
                .map(String::as_str)
                .unwrap_or("(no question)");
            let selected = app.selected == Some(i);
            let marker = if selected { "▌ " } else { "  " };
            let q_style = if selected {
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::White)
                    .bg(Color::Indexed(238))
            } else {
                Style::default()
            };
            let q_line = Line::from(vec![
                Span::raw(marker),
                Span::styled(q.to_string(), q_style),
            ]);
            let rows = wrapped_row_count(&q_line, inner_width);
            for _ in 0..rows {
                row_of_item.push(i);
            }
            lines.push(q_line);

            // The unselected score line is intentionally dim (DarkGray); once
            // selected it sits on the Indexed(238) highlight, where DarkGray
            // text would blend into the background — flip it to White (the
            // opposite end of the contrast range) so it stays readable
            // without changing the highlight color itself.
            let score_style = if selected {
                Style::default().fg(Color::White).bg(Color::Indexed(238))
            } else {
                Style::default().fg(Color::DarkGray)
            };
            row_of_item.push(i);
            lines.push(Line::styled(format!("    {score:.4}"), score_style));
        }
    }

    let total_rows = row_of_item.len() as u16;
    if app.scroll_follow_selection
        && let Some(sel) = app.selected
    {
        let start = row_of_item.iter().position(|&x| x == sel);
        let end = row_of_item.iter().rposition(|&x| x == sel);
        if let (Some(start), Some(end)) = (start, end) {
            let (start, end) = (start as u16, end as u16);
            if start < app.results_scroll {
                app.results_scroll = start;
            }
            if inner_height > 0 && end >= app.results_scroll + inner_height {
                app.results_scroll = end + 1 - inner_height;
            }
        }
    }
    app.results_scroll = app
        .results_scroll
        .min(total_rows.saturating_sub(inner_height));
    app.results_row_targets = row_of_item;

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.results_scroll, 0));
    frame.render_widget(paragraph, area);
}

/// Owns the parallel `lines`/`targets` vectors while building Detail text so
/// every push keeps them aligned (`targets[i]` is the jump target, if any,
/// for `lines[i]`).
#[derive(Default)]
struct LineBuilder {
    lines: Vec<Line<'static>>,
    targets: Vec<Option<u32>>,
}

impl LineBuilder {
    fn push(&mut self, line: Line<'static>) {
        self.lines.push(line);
        self.targets.push(None);
    }

    fn push_target(&mut self, line: Line<'static>, target: Option<u32>) {
        self.lines.push(line);
        self.targets.push(target);
    }

    fn finish(self) -> (Vec<Line<'static>>, Vec<Option<u32>>) {
        (self.lines, self.targets)
    }
}

fn detail_lines(app: &App) -> (Vec<Line<'static>>, Vec<Option<u32>>) {
    let mut b = LineBuilder::default();

    if app.is_command_input() {
        let heading = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);

        // Typing `/semantic` opens a screen explaining each model, mirroring
        // how `/` alone lists the commands.
        if app.input.trim_start().starts_with("/semantic") {
            b.push(Line::styled("Semantic model  (/semantic <name>)", heading));
            b.push(Line::raw(""));
            for (name, desc) in MODEL_DESCRIPTIONS {
                b.push(Line::from(vec![
                    Span::styled(
                        format!("{name:<8}"),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw((*desc).to_string()),
                ]));
            }
            b.push(Line::raw(""));
            b.push(Line::styled(
                "Enter to apply — e.g.  /semantic max",
                Style::default().fg(Color::DarkGray),
            ));
            if let Some(err) = &app.command_error {
                b.push(Line::raw(""));
                b.push(Line::styled(
                    format!("⚠ {err}"),
                    Style::default().fg(Color::Yellow),
                ));
            }
            return b.finish();
        }

        b.push(Line::styled("Commands", heading));
        b.push(Line::raw("/semantic                  switch the embedding"));
        b.push(Line::raw("/config                    settings"));
        b.push(Line::raw("/help                      shortcut reference"));
        b.push(Line::raw("/exit (or /quit)           quit"));
        if let Some(err) = &app.command_error {
            b.push(Line::raw(""));
            b.push(Line::styled(
                format!("⚠ {err}"),
                Style::default().fg(Color::Yellow),
            ));
        }
        return b.finish();
    }

    if let Some(err) = &app.data_error {
        b.push(Line::styled(
            "Failed to load data:",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
        b.push(Line::raw(err.clone()));
        return b.finish();
    }
    let Some(data) = &app.data else {
        return b.finish();
    };
    let Some(doc) = app.selected_doc() else {
        let hint = if app.cfg.model.is_enabled() {
            "Type to search (BM25), press Enter for semantic + RRF."
        } else {
            "Type to search (BM25-only; semantic is disabled)."
        };
        b.push(Line::raw(hint));
        if let Some(err) = &app.command_error {
            b.push(Line::raw(""));
            b.push(Line::styled(
                format!("⚠ {err}"),
                Style::default().fg(Color::Yellow),
            ));
        }
        if !app.warnings.is_empty() {
            b.push(Line::raw(""));
            for w in &app.warnings {
                b.push(Line::styled(
                    format!("⚠ {w}"),
                    Style::default().fg(Color::Yellow),
                ));
            }
        }
        return b.finish();
    };
    let record = &data.corpus.records[doc as usize];
    let dim = Style::default().fg(Color::DarkGray);
    let heading = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    b.push(Line::styled(
        record.questions.join(" / "),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    b.push(Line::styled(
        format!(
            "{} · {} · priority {}",
            record.category.join(", "),
            record.difficulty,
            record.effective_priority()
        ),
        dim,
    ));
    b.push(Line::styled(
        format!("id {} · updated {}", record.id, record.updated_at),
        dim,
    ));
    b.push(Line::raw(""));
    if !record.description.is_empty() {
        for l in record.description.lines() {
            b.push(Line::raw(l.to_string()));
        }
        b.push(Line::raw(""));
    }
    b.push(Line::styled("Answer", heading));
    for l in record.answer.lines() {
        b.push(Line::raw(l.to_string()));
    }
    if !record.keywords.is_empty() {
        b.push(Line::raw(""));
        b.push(Line::styled(
            format!("keywords: {}", record.keywords.join(", ")),
            dim,
        ));
    }
    if !record.synonyms.is_empty() {
        b.push(Line::styled(
            format!("synonyms: {}", record.synonyms.join(", ")),
            dim,
        ));
    }
    if !record.related.is_empty() {
        b.push(Line::raw(""));
        b.push(Line::styled(
            "Related (Tab to browse, Enter to jump)",
            heading,
        ));
        for (i, id) in record.related.iter().enumerate() {
            let target = data
                .corpus
                .records
                .iter()
                .position(|r| &r.id == id)
                .map(|idx| idx as u32);
            let label = target
                .and_then(|idx| data.corpus.records[idx as usize].questions.first().cloned())
                .unwrap_or_else(|| id.clone());
            let focused = app.pane_focus == PaneFocus::Related && app.related_selected == Some(i);
            let marker = if focused { "▸ " } else { "  " };
            let style = if focused {
                Style::default().fg(Color::White).bg(Color::Cyan)
            } else if target.is_some() {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                dim
            };
            b.push_target(
                Line::from(vec![Span::raw(marker), Span::styled(label, style)]),
                target,
            );
        }
    }
    b.finish()
}

fn draw_detail(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::bordered().title(" Detail ");
    let inner = block.inner(area);
    let inner_width = inner.width.max(1);
    let inner_height = inner.height;

    let (lines, line_targets) = detail_lines(app);

    let mut row_targets: Vec<Option<u32>> = Vec::new();
    for (line, target) in lines.iter().zip(line_targets.iter()) {
        let rows = wrapped_row_count(line, inner_width);
        for _ in 0..rows {
            row_targets.push(*target);
        }
    }
    let total_rows = row_targets.len() as u16;
    app.detail_scroll = app
        .detail_scroll
        .min(total_rows.saturating_sub(inner_height));
    app.detail_row_targets = row_targets;

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));
    frame.render_widget(paragraph, area);
}

fn draw_overlay(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.overlay {
        Overlay::Help => draw_help(frame, area),
        Overlay::Config => draw_config(frame, app, area),
        Overlay::None => {}
    }
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let heading = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);
    let lines = vec![
        Line::styled("Keyboard", heading),
        Line::raw("  type              instant BM25 search"),
        Line::raw("  Enter             semantic + RRF fusion (or run a /command)"),
        Line::raw("  ↑ ↓ / Ctrl-P/N    move selection in Results"),
        Line::raw("  PgUp / PgDn       scroll Detail"),
        Line::raw("  Tab               browse this item's Related list; ↑↓ pick, Enter jumps"),
        Line::raw("  Esc               close this screen / clear query / quit"),
        Line::raw("  Ctrl-C / Ctrl-Q   quit"),
        Line::raw(""),
        Line::styled("Mouse", heading),
        Line::raw("  wheel over Results     scroll the list (selection unchanged)"),
        Line::raw("  wheel over Detail      scroll the text"),
        Line::raw("  click a result         select it"),
        Line::raw("  click a Related item   jump to it"),
        Line::raw(""),
        Line::styled("Commands", heading),
        Line::raw("  /semantic                 switch the embedding (type it for details)"),
        Line::raw("  /config                   settings screen"),
        Line::raw("  /help                     this screen"),
        Line::raw("  /exit (or /quit)          quit"),
        Line::raw(""),
        Line::styled("press any key to close", dim),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(" Help "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_config(frame: &mut Frame, app: &mut App, area: Rect) {
    let heading = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);
    let mut lines = vec![
        Line::styled(
            "Settings  (↑↓ select · ←→/Enter change · PgUp/PgDn scroll · Esc close)",
            heading,
        ),
        Line::raw(""),
    ];

    let focused = app.focused_config_field();
    let field_row = |label: &str, value: String, is_focused: bool| -> Line<'static> {
        let marker = if is_focused { "▸ " } else { "  " };
        let style = if is_focused {
            Style::default().fg(Color::White).bg(Color::Cyan)
        } else {
            Style::default()
        };
        Line::from(vec![
            Span::raw(marker),
            Span::styled(format!("{label:<16}"), style),
            Span::raw("  "),
            Span::styled(value, style),
        ])
    };
    // The per-model descriptions live under "Semantic status" below, so the
    // model value here is just the current selection.
    lines.push(field_row(
        "Semantic model",
        app.cfg.model.label().to_string(),
        focused == ConfigField::Model,
    ));
    lines.push(field_row(
        "Offline mode",
        if app.cfg.offline { "on" } else { "off" }.to_string(),
        focused == ConfigField::Offline,
    ));

    // ── Model weights (custom mode only, --debug) ─ shown above Semantic status.
    if app.cfg.debug && app.cfg.model == ModelSel::Custom {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Model weights  (←/→ adjust · Enter reset)",
            heading,
        ));
        let weight_row = |label: &str, val: f64, is_focused: bool| -> Line<'static> {
            let marker = if is_focused { "▸ " } else { "  " };
            let style = if is_focused {
                Style::default().fg(Color::White).bg(Color::Cyan)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::raw(marker),
                Span::styled(format!("{label:<10}"), style),
                Span::raw("  "),
                Span::styled(format!("×{val:.1}"), style),
            ])
        };
        let w = app.cfg.weights;
        lines.push(weight_row(
            "BM25",
            w.bm25,
            focused == ConfigField::WeightBm25,
        ));
        lines.push(weight_row(
            "e5-small",
            w.small,
            focused == ConfigField::WeightSmall,
        ));
        lines.push(weight_row(
            "e5-large",
            w.large,
            focused == ConfigField::WeightLarge,
        ));

        // Live preview: re-fuse the last query's snapshot with the current
        // weights every frame, so ←/→ shows the re-ranking in real time.
        lines.push(Line::raw(""));
        if app.snap_query.is_empty() || app.snap_semantic.len() < 2 {
            lines.push(Line::styled(
                "  preview: run a search in max/custom mode to see live re-ranking",
                dim,
            ));
        } else {
            lines.push(Line::styled(
                format!("  Preview · top 5 for \"{}\"", app.snap_query),
                dim,
            ));
            let preview = app.custom_preview();
            if let Some(data) = &app.data {
                for (rank, &(doc, score)) in preview.iter().take(5).enumerate() {
                    let q = data
                        .corpus
                        .records
                        .get(doc as usize)
                        .and_then(|r| r.questions.first())
                        .map(String::as_str)
                        .unwrap_or("(no question)");
                    lines.push(Line::raw(format!("  {}. {q}  ({score:.4})", rank + 1)));
                }
            }
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled("Semantic status", heading));
    let sem_status = match &app.sem_state {
        SemState::Off => "not started".to_string(),
        SemState::Init => "loading…".to_string(),
        SemState::Ready => "ready".to_string(),
        SemState::Disabled => "disabled (BM25-only)".to_string(),
        SemState::Failed { invariant } => {
            let err = app.sem_error.clone().unwrap_or_default();
            if *invariant {
                format!("INVARIANT BROKEN: {err}")
            } else {
                format!("unavailable: {err}")
            }
        }
    };
    lines.push(Line::raw(format!("  {sem_status}")));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "  Models (/semantic <name> or ←→ to cycle)",
        dim,
    ));
    for (name, desc) in MODEL_DESCRIPTIONS {
        lines.push(Line::from(vec![
            Span::styled(format!("  {name:<7}"), Style::default().fg(Color::Cyan)),
            Span::styled((*desc).to_string(), dim),
        ]));
    }
    if app.cfg.debug {
        lines.push(Line::from(vec![
            Span::styled("  custom ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "like max, but with tunable per-model weights (--debug)".to_string(),
                dim,
            ),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Read-only (set at launch via flags/env)",
        heading,
    ));
    lines.push(Line::styled(
        format!("  base URL     {}", app.cfg.base_url),
        dim,
    ));
    lines.push(Line::styled(
        format!("  cache dir    {}", app.cfg.cache_root.display()),
        dim,
    ));
    lines.push(Line::styled(
        format!("  tokenizer    {}", crate::config::TOKENIZER_TAG),
        dim,
    ));
    if !app.warnings.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("Warnings", heading));
        for w in &app.warnings {
            lines.push(Line::styled(
                format!("  ⚠ {w}"),
                Style::default().fg(Color::Yellow),
            ));
        }
    }

    let block = Block::bordered().title(" Config ");
    let inner = block.inner(area);
    let total_rows: u16 = lines
        .iter()
        .map(|l| wrapped_row_count(l, inner.width.max(1)))
        .sum();
    app.config_scroll = app
        .config_scroll
        .min(total_rows.saturating_sub(inner.height));

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((app.config_scroll, 0)),
        area,
    );
}

/// The fixed left of the status bar: the spinner/ready dot plus the semantic
/// state (`semantic: ready (small)` etc.). Everything to the *right* of this
/// scrolls as a marquee (see `render_status`), because it can outgrow a narrow
/// terminal and would otherwise be clipped.
fn status_left_spans(app: &App) -> Vec<Span<'static>> {
    let mut spans: Vec<Span> = Vec::new();

    // Spinner while anything is in flight (§11): phase text for slow steps,
    // whimsical verbs for short waits.
    if let Some((label, started)) = &app.phase {
        spans.push(Span::styled(
            spinner::line(started.elapsed(), Some(label.as_str()), 0),
            Style::default().fg(Color::Cyan),
        ));
    } else if let Some((seq, started)) = &app.pending_sem {
        spans.push(Span::styled(
            spinner::line(started.elapsed(), None, *seq),
            Style::default().fg(Color::Cyan),
        ));
    } else {
        spans.push(Span::styled("● ready", Style::default().fg(Color::Green)));
    }

    spans.push(Span::raw("  ·  "));
    let (sem_text, sem_style) = match &app.sem_state {
        SemState::Off | SemState::Init => (
            "semantic: loading".to_string(),
            Style::default().fg(Color::Yellow),
        ),
        SemState::Disabled => (
            "semantic: off (BM25-only)".to_string(),
            Style::default().fg(Color::DarkGray),
        ),
        SemState::Ready => (
            format!("semantic: ready ({})", app.cfg.model.label()),
            Style::default().fg(Color::Green),
        ),
        SemState::Failed { invariant } => {
            // Keep the fixed part short: a URL/long detail would defeat the
            // marquee. The full error stays in Detail and /config.
            let text = if *invariant {
                "SEMANTIC INVARIANT BROKEN".to_string()
            } else {
                "semantic: off (error)".to_string()
            };
            (
                text,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        }
    };
    spans.push(Span::styled(sem_text, sem_style));
    spans
}

/// The scrolling right of the status bar: an optional warning/error followed
/// by the keyboard-shortcut hint. URLs are stripped here (but not in Detail or
/// `/config`) so a fetch error can't blow the status bar out to an unreadable
/// length. Returns the text plus the style to render it in.
fn status_tail(app: &App) -> (String, Style) {
    let mut parts: Vec<String> = Vec::new();
    let mut warn = false;
    if let Some(err) = &app.command_error {
        parts.push(format!("⚠ {}", strip_urls(err)));
        warn = true;
    } else if let Some(w) = app.warnings.first() {
        parts.push(format!("⚠ {}", strip_urls(w)));
        warn = true;
    }
    parts.push(
        "Enter search · ↑↓ select · Tab related · PgUp/PgDn scroll · /help · Esc quit".to_string(),
    );
    let style = if warn {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    (parts.join("  ·  "), style)
}

/// Render the status bar: fixed left, then the marqueed tail sized to whatever
/// width remains. If the tail already fits, it's shown static (no scrolling).
fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = status_left_spans(app);
    let left_w: usize = spans.iter().map(|s| s.content.width()).sum();

    let sep = "  ·  ";
    let (tail, tail_style) = status_tail(app);
    let window = (area.width as usize).saturating_sub(left_w + sep.width());
    if window > 0 && !tail.is_empty() {
        let offset = (app.app_start.elapsed().as_millis() / MARQUEE_STEP_MS) as usize;
        spans.push(Span::raw(sep));
        spans.push(Span::styled(marquee(&tail, window, offset), tail_style));
    }

    frame.render_widget(Line::from(spans), area);
}

/// Replace any `http://…`/`https://…` token (up to the next whitespace or a
/// closing `)"'`) with `…`, so fetch-error messages stay short in the status
/// bar without losing the surrounding context.
fn strip_urls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find("http") {
        let after = &rest[pos..];
        if after.starts_with("http://") || after.starts_with("https://") {
            out.push_str(&rest[..pos]);
            out.push('…');
            let end = after
                .find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '\''))
                .unwrap_or(after.len());
            rest = &after[end..];
        } else {
            // "http" not part of a URL scheme — keep it and move past it.
            out.push_str(&rest[..pos + 4]);
            rest = &rest[pos + 4..];
        }
    }
    out.push_str(rest);
    out
}

/// A one-line horizontal marquee. If `text` fits in `window` columns it's
/// returned unchanged; otherwise it's looped (with `MARQUEE_GAP`) and a
/// `window`-wide slice starting `offset` characters in is returned, so callers
/// ticking `offset` over time get a scrolling "LED sign". Width-aware for CJK.
fn marquee(text: &str, window: usize, offset: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    if window == 0 {
        return String::new();
    }
    if text.width() <= window {
        return text.to_string();
    }
    let full: Vec<char> = format!("{text}{MARQUEE_GAP}").chars().collect();
    let n = full.len();
    let start = offset % n;
    let mut out = String::new();
    let mut w = 0usize;
    let mut i = 0usize;
    // One revolution of `full` is wider than `window`, so `w` reaches `window`
    // before `i` wraps — the `i < n` bound just guarantees termination.
    while w < window && i < n {
        let ch = full[(start + i) % n];
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > window {
            break;
        }
        out.push(ch);
        w += cw;
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_row_count_fits_on_one_row_when_short() {
        let line = Line::raw("hello");
        assert_eq!(wrapped_row_count(&line, 20), 1);
    }

    #[test]
    fn wrapped_row_count_wraps_ascii_text() {
        let line = Line::raw("a b c d e f g h i j k l m n o p");
        assert!(wrapped_row_count(&line, 5) > 1);
    }

    #[test]
    fn wrapped_row_count_wraps_cjk_text_without_spaces() {
        // Japanese has no spaces; a naive ASCII word-wrapper would treat this
        // as one unbreakable "word" and never wrap it. ratatui's own wrap
        // must still break it across rows at a narrow width.
        let line = Line::raw("電磁誘導と静電気力についての詳しい説明をここに書きます");
        assert!(wrapped_row_count(&line, 10) > 1);
    }

    #[test]
    fn wrapped_row_count_never_reports_zero() {
        let line = Line::raw("");
        assert_eq!(wrapped_row_count(&line, 10), 1);
        assert_eq!(wrapped_row_count(&line, 0), 1);
    }

    #[test]
    fn strip_urls_replaces_url_tokens_but_keeps_context() {
        assert_eq!(
            strip_urls(
                "offline? (error sending request for url (https://raw.githubusercontent.com/legrs/physics_notes/x/version.json): connection refused); using cached data"
            ),
            "offline? (error sending request for url (…): connection refused); using cached data"
        );
        assert_eq!(
            strip_urls("plain http status text"),
            "plain http status text"
        );
        assert_eq!(strip_urls("see http://a.b/c now"), "see … now");
        assert_eq!(strip_urls("no urls here"), "no urls here");
    }

    #[test]
    fn marquee_returns_text_unchanged_when_it_fits() {
        assert_eq!(marquee("short", 20, 0), "short");
        assert_eq!(marquee("short", 20, 999), "short");
    }

    #[test]
    fn marquee_scrolls_and_wraps_when_too_long() {
        let text = "abcdefghij"; // width 10
        let w = 4;
        let a = marquee(text, w, 0);
        assert_eq!(a.width(), w);
        assert_eq!(a, "abcd");
        // Advancing the offset scrolls the window forward.
        assert_eq!(marquee(text, w, 1), "bcde");
        // Offset past the looped length wraps around without panicking.
        let wrapped = marquee(text, w, text.chars().count() + MARQUEE_GAP.chars().count());
        assert_eq!(wrapped, "abcd");
    }

    #[test]
    fn marquee_zero_window_is_empty() {
        assert_eq!(marquee("anything", 0, 0), "");
    }

    #[test]
    fn bump_weight_steps_and_clamps() {
        let mut w = 2.0;
        bump_weight(&mut w, 1.0);
        assert!((w - 2.1).abs() < 1e-9);
        bump_weight(&mut w, -1.0);
        assert!((w - 2.0).abs() < 1e-9);
        // Clamp at the bounds; no float drift below MIN or above MAX.
        w = CustomWeights::MIN;
        bump_weight(&mut w, -1.0);
        assert_eq!(w, CustomWeights::MIN);
        w = CustomWeights::MAX;
        bump_weight(&mut w, 1.0);
        assert_eq!(w, CustomWeights::MAX);
    }
}
