//! Interactive TUI (ratatui + crossterm, CLAUDE.md §3, §11).
//!
//! 2-tier UX (§6): every keystroke re-runs BM25 instantly; submit (Enter) or
//! a short typing debounce triggers the semantic ranking, RRF-fused when it
//! arrives. All heavy work (fetch, model load, query embedding) runs on
//! background tasks; the render loop only animates the spinner (§11).

use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::config::Config;
use crate::engine::{hybrid, Engine, SemanticEngine};
use crate::query::prepare_query;
use crate::semantic::SemanticError;
use crate::spinner;

const SEMANTIC_DEBOUNCE: Duration = Duration::from_millis(500);
const DETAIL_SCROLL_STEP: u16 = 5;

enum AppMsg {
    Progress(String),
    Data(Box<Result<Engine, String>>),
    SemanticUp,
    SemanticDown {
        error: String,
        invariant: bool,
    },
    SemanticRanked {
        seq: u64,
        ranked: Vec<(u32, f64)>,
    },
    SemanticQueryFailed {
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
    Failed {
        invariant: bool,
    },
}

#[derive(Clone, Copy, PartialEq)]
enum ResultsMode {
    Bm25,
    Hybrid,
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
    /// Latest fully-boosted BM25 list (what RRF consumes).
    bm25_results: Vec<(u32, f64)>,
    /// What the list shows (BM25 or RRF-merged).
    results: Vec<(u32, f64)>,
    results_mode: ResultsMode,
    list_state: ListState,
    detail_scroll: u16,
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
            bm25_results: Vec::new(),
            results: Vec::new(),
            results_mode: ResultsMode::Bm25,
            list_state: ListState::default(),
            detail_scroll: 0,
            seq: 0,
            last_requested_seq: 0,
            last_input_at: Instant::now(),
            force_semantic: false,
            pending_sem: None,
            phase: Some(("Starting…".to_string(), Instant::now())),
            warnings: Vec::new(),
            q_lower: String::new(),
            should_quit: false,
        }
    }

    fn selected_doc(&self) -> Option<u32> {
        self.list_state
            .selected()
            .and_then(|i| self.results.get(i))
            .map(|&(d, _)| d)
    }

    fn refresh_bm25(&mut self) {
        self.seq += 1;
        self.last_input_at = Instant::now();
        self.pending_sem = None;
        self.detail_scroll = 0;
        self.q_lower = prepare_query(&self.input);
        let Some(data) = &self.data else {
            return;
        };
        if self.q_lower.is_empty() {
            self.bm25_results.clear();
            self.results.clear();
            self.list_state.select(None);
            self.results_mode = ResultsMode::Bm25;
            return;
        }
        self.bm25_results = data.bm25(&self.q_lower);
        self.results = self.bm25_results.clone();
        self.results_mode = ResultsMode::Bm25;
        self.list_state.select(if self.results.is_empty() {
            None
        } else {
            Some(0)
        });
    }

    fn maybe_request_semantic(&mut self) {
        if self.sem_state != SemState::Ready || self.q_lower.is_empty() {
            return;
        }
        if self.seq <= self.last_requested_seq {
            return;
        }
        if !self.force_semantic && self.last_input_at.elapsed() < SEMANTIC_DEBOUNCE {
            return;
        }
        if let Some(tx) = &self.sem_tx {
            if tx
                .send(SemReq {
                    seq: self.seq,
                    q: self.q_lower.clone(),
                })
                .is_ok()
            {
                self.last_requested_seq = self.seq;
                self.pending_sem = Some((self.seq, Instant::now()));
            }
        }
        self.force_semantic = false;
    }

    fn handle_msg(&mut self, msg: AppMsg, rx_sem: &mut Option<std_mpsc::Receiver<SemReq>>) {
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
                    self.phase = None;
                    self.data = Some(bundle);
                    self.sem_state = SemState::Init;
                    self.phase = Some(("Loading semantic model…".to_string(), Instant::now()));
                    // Hand the worker its request channel.
                    let (tx, rx) = std_mpsc::channel();
                    self.sem_tx = Some(tx);
                    *rx_sem = Some(rx);
                    self.refresh_bm25();
                }
                Err(e) => {
                    self.phase = None;
                    self.data_error = Some(e);
                }
            },
            AppMsg::SemanticUp => {
                self.sem_state = SemState::Ready;
                self.phase = None;
            }
            AppMsg::SemanticDown { error, invariant } => {
                self.sem_state = SemState::Failed { invariant };
                self.sem_error = Some(error);
                self.phase = None;
            }
            AppMsg::SemanticRanked { seq, ranked } => {
                if let Some((pseq, _)) = self.pending_sem {
                    if seq >= pseq {
                        self.pending_sem = None;
                    }
                }
                if seq == self.seq {
                    self.results = hybrid(&self.bm25_results, &ranked);
                    self.results_mode = ResultsMode::Hybrid;
                    self.list_state.select(if self.results.is_empty() {
                        None
                    } else {
                        Some(0)
                    });
                    self.detail_scroll = 0;
                }
            }
            AppMsg::SemanticQueryFailed {
                seq,
                error,
                invariant,
            } => {
                if let Some((pseq, _)) = self.pending_sem {
                    if seq >= pseq {
                        self.pending_sem = None;
                    }
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
        match key.code {
            KeyCode::Char('c') | KeyCode::Char('q') if ctrl => self.should_quit = true,
            KeyCode::Esc => {
                if self.input.is_empty() {
                    self.should_quit = true;
                } else {
                    self.input.clear();
                    self.cursor = 0;
                    self.refresh_bm25();
                }
            }
            KeyCode::Enter => {
                // Submit: semantic + RRF (fires immediately, or as soon as
                // the model is ready).
                self.force_semantic = true;
                // Re-arm even if this seq was already requested (e.g. the
                // response was stale or lost).
                self.last_requested_seq = self.seq.saturating_sub(1).min(self.last_requested_seq);
            }
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
            KeyCode::Char(c) if !ctrl && !c.is_control() => {
                let idx = byte_index(&self.input, self.cursor);
                self.input.insert(idx, c);
                self.cursor += 1;
                self.refresh_bm25();
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: i64) {
        if self.results.is_empty() {
            return;
        }
        let cur = self.list_state.selected().unwrap_or(0) as i64;
        let next = (cur + delta).clamp(0, self.results.len() as i64 - 1) as usize;
        self.list_state.select(Some(next));
        self.detail_scroll = 0;
    }
}

fn byte_index(s: &str, char_offset: usize) -> usize {
    s.char_indices()
        .nth(char_offset)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// The semantic worker owns the `SemanticEngine` on its own thread; the
/// model loads (and on first run, downloads) behind the spinner while BM25
/// is already usable. All ranking logic lives in `engine` — this thread is
/// only channel plumbing.
fn semantic_worker(
    cfg: Config,
    engine: Engine,
    req_rx: std_mpsc::Receiver<SemReq>,
    tx: std_mpsc::Sender<AppMsg>,
) {
    let mut sem = match SemanticEngine::load(&cfg, engine.corpus.clone()) {
        Ok(sem) => sem,
        Err(e) => {
            let invariant = matches!(e, SemanticError::Invariant(_));
            let _ = tx.send(AppMsg::SemanticDown {
                error: e.to_string(),
                invariant,
            });
            return;
        }
    };
    let _ = tx.send(AppMsg::SemanticUp);

    while let Ok(mut req) = req_rx.recv() {
        // Collapse a burst of requests down to the newest one.
        while let Ok(newer) = req_rx.try_recv() {
            req = newer;
        }
        let msg = match sem.rank(&req.q) {
            Ok(ranked) => AppMsg::SemanticRanked {
                seq: req.seq,
                ranked,
            },
            Err(e) => AppMsg::SemanticQueryFailed {
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
    let result = run_loop(&mut terminal, &mut app, rx, tx);
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rx: std_mpsc::Receiver<AppMsg>,
    tx: std_mpsc::Sender<AppMsg>,
) -> Result<()> {
    // The semantic worker is spawned lazily once data arrives.
    let mut sem_req_rx: Option<std_mpsc::Receiver<SemReq>> = None;

    loop {
        while let Ok(msg) = rx.try_recv() {
            app.handle_msg(msg, &mut sem_req_rx);
        }
        if let Some(req_rx) = sem_req_rx.take() {
            if let Some(engine) = &app.data {
                let cfg = app.cfg.clone();
                let engine = engine.clone();
                let tx = tx.clone();
                std::thread::spawn(move || semantic_worker(cfg, engine, req_rx, tx));
            }
        }
        app.maybe_request_semantic();

        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(Duration::from_millis(spinner::FRAME_MS))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
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

    // ── body ───────────────────────────────────────────────────────────
    let [list_area, detail_area] =
        Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).areas(body);

    let mode_label = match app.results_mode {
        ResultsMode::Bm25 => "BM25",
        ResultsMode::Hybrid => "Hybrid (BM25+semantic RRF)",
    };
    let title = format!(" Results · {} · {} ", mode_label, app.results.len());
    let items: Vec<ListItem> = app
        .results
        .iter()
        .map(|&(doc, score)| {
            let corpus = &app.data.as_ref().unwrap().corpus;
            let record = &corpus.records[doc as usize];
            let q = record
                .questions
                .first()
                .map(String::as_str)
                .unwrap_or("(no question)");
            ListItem::new(Line::from(vec![
                Span::raw(q.to_string()),
                Span::styled(
                    format!("  {score:.4}"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(Block::bordered().title(title))
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(238))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▌ ");
    frame.render_stateful_widget(list, list_area, &mut app.list_state);

    // ── detail ─────────────────────────────────────────────────────────
    let detail = detail_text(app);
    frame.render_widget(
        Paragraph::new(detail)
            .block(Block::bordered().title(" Detail "))
            .wrap(Wrap { trim: false })
            .scroll((app.detail_scroll, 0)),
        detail_area,
    );

    // ── status ─────────────────────────────────────────────────────────
    frame.render_widget(status_line(app), status_area);
}

fn detail_text(app: &App) -> Text<'static> {
    if let Some(err) = &app.data_error {
        return Text::from(vec![
            Line::styled(
                "Failed to load data:",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Line::raw(err.clone()),
        ]);
    }
    let Some(data) = &app.data else {
        return Text::raw("");
    };
    let Some(doc) = app.selected_doc() else {
        let mut lines = vec![Line::raw(
            "Type to search (BM25), press Enter for semantic + RRF.",
        )];
        if !app.warnings.is_empty() {
            lines.push(Line::raw(""));
            for w in &app.warnings {
                lines.push(Line::styled(
                    format!("⚠ {w}"),
                    Style::default().fg(Color::Yellow),
                ));
            }
        }
        return Text::from(lines);
    };
    let record = &data.corpus.records[doc as usize];
    let dim = Style::default().fg(Color::DarkGray);
    let heading = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::styled(
        record.questions.join(" / "),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::styled(
        format!(
            "{} · {} · priority {}",
            record.category.join(", "),
            record.difficulty,
            record.effective_priority()
        ),
        dim,
    ));
    lines.push(Line::styled(
        format!("id {} · updated {}", record.id, record.updated_at),
        dim,
    ));
    lines.push(Line::raw(""));
    if !record.description.is_empty() {
        for l in record.description.lines() {
            lines.push(Line::raw(l.to_string()));
        }
        lines.push(Line::raw(""));
    }
    lines.push(Line::styled("Answer", heading));
    for l in record.answer.lines() {
        lines.push(Line::raw(l.to_string()));
    }
    if !record.keywords.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("keywords: {}", record.keywords.join(", ")),
            dim,
        ));
    }
    if !record.synonyms.is_empty() {
        lines.push(Line::styled(
            format!("synonyms: {}", record.synonyms.join(", ")),
            dim,
        ));
    }
    if !record.related.is_empty() {
        let names: Vec<String> = record
            .related
            .iter()
            .map(|id| {
                data.corpus
                    .records
                    .iter()
                    .find(|r| &r.id == id)
                    .and_then(|r| r.questions.first().cloned())
                    .unwrap_or_else(|| id.clone())
            })
            .collect();
        lines.push(Line::styled(format!("related: {}", names.join(" · ")), dim));
    }
    Text::from(lines)
}

fn status_line(app: &App) -> Line<'static> {
    let mut spans: Vec<Span> = Vec::new();

    // Left: spinner while anything is in flight (§11): phase text for slow
    // steps, whimsical verbs for short waits.
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
        SemState::Ready => (
            format!("semantic: ready ({})", app.cfg.model.embeddings_key()),
            Style::default().fg(Color::Green),
        ),
        SemState::Failed { invariant } => {
            let err = app.sem_error.clone().unwrap_or_default();
            let text = if *invariant {
                format!("SEMANTIC INVARIANT BROKEN: {err}")
            } else {
                format!("semantic off: {err}")
            };
            (
                text,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        }
    };
    spans.push(Span::styled(sem_text, sem_style));

    if let Some(w) = app.warnings.first() {
        spans.push(Span::raw("  ·  "));
        spans.push(Span::styled(
            format!("⚠ {w}"),
            Style::default().fg(Color::Yellow),
        ));
    }

    spans.push(Span::raw("  ·  "));
    spans.push(Span::styled(
        "Enter semantic · ↑↓ select · PgUp/PgDn scroll · Esc quit",
        Style::default().fg(Color::DarkGray),
    ));
    Line::from(spans)
}
