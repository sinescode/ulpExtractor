use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::{self};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::extractor::{self, ExtractProgress};

// ── App State ──────────────────────────────────────────────────────────

enum Screen {
    Input,
    Processing(ProcessState),
    Done(DoneState),
}

struct ProcessState {
    domain: String,
    input_label: String,
    progress: Arc<ExtractProgress>,
    cancelled: Arc<AtomicBool>,
    done: bool,
    matched: usize,
    total: usize,
    duration_ms: u64,
}

struct DoneState {
    domain: String,
    matched: usize,
    total: usize,
    duration_ms: u64,
    output: String,
    cancelled: bool,
}

struct FilePicker {
    current_dir: PathBuf,
    entries: Vec<FsEntry>,
    selected: usize,
    scroll: usize,
}

struct FsEntry {
    name: String,
    is_dir: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Domain,
    Threads,
    Divider,
    Input,
    Output,
    Start,
}

struct App {
    screen: Screen,
    domain: String,
    threads: String,
    divider: String,
    input_path: String,
    output_path: String,
    focus: Focus,
    file_picker: Option<FilePicker>,
    picker_for: Option<PickerTarget>,
    status: String,
    should_quit: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum PickerTarget {
    Input,
    Output,
}

// ── Public API ─────────────────────────────────────────────────────────

pub fn run() -> io::Result<()> {
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen)?;
    enable_raw_mode()?;

    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        orig_hook(info);
    }));

    let backend = ratatui::backend::CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    terminal.hide_cursor()?;

    let result = run_app(&mut terminal);

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    result
}

// ── App Loop ───────────────────────────────────────────────────────────

fn run_app(terminal: &mut Terminal<impl ratatui::backend::Backend>) -> io::Result<()> {
    let mut app = App::new();

    loop {
        terminal.draw(|f| render(f, &mut app))?;
        handle_input(&mut app)?;
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

impl App {
    fn new() -> Self {
        Self {
            screen: Screen::Input,
            domain: String::new(),
            threads: String::from("4"),
            divider: String::from(":"),
            input_path: String::new(),
            output_path: String::from("output.txt"),
            focus: Focus::Domain,
            file_picker: None,
            picker_for: None,
            status: String::new(),
            should_quit: false,
        }
    }

    fn focused_field_mut(&mut self) -> &mut String {
        match self.focus {
            Focus::Domain => &mut self.domain,
            Focus::Threads => &mut self.threads,
            Focus::Divider => &mut self.divider,
            Focus::Input => &mut self.input_path,
            Focus::Output => &mut self.output_path,
            Focus::Start => &mut self.domain, // fallback
        }
    }
}

// ── Input Handling ─────────────────────────────────────────────────────

fn handle_input(app: &mut App) -> io::Result<()> {
    if !event::poll(std::time::Duration::from_millis(16))? {
        // Check for processing completion
        if let Screen::Processing(ref state) = app.screen {
            let p = state.progress.processed.load(Ordering::Relaxed);
            let m = state.progress.matched.load(Ordering::Relaxed);
            let t = state.progress.total.load(Ordering::Relaxed);
            if p >= t && t > 0 {
                app.finish_processing(m, t, false);
            }
        }
        return Ok(());
    }

    let event = event::read()?;
    let Event::Key(key) = event else { return Ok(()) };
    if key.kind == KeyEventKind::Release {
        return Ok(());
    }

    if app.file_picker.is_some() {
        return handle_file_picker(app, key.code);
    }

    match app.screen {
        Screen::Input => handle_input_screen(app, key.code),
        Screen::Processing(_) => handle_processing_screen(app, key.code),
        Screen::Done(_) => handle_done_screen(app, key.code),
    }
}

fn handle_input_screen(app: &mut App, code: KeyCode) -> io::Result<()> {
    match code {
        KeyCode::Esc => app.should_quit = true,
        KeyCode::Tab => app.focus = next_focus(&app.focus),
        KeyCode::BackTab => app.focus = prev_focus(&app.focus),
        KeyCode::Enter if matches!(app.focus, Focus::Start) => {
            app.start_extraction()?;
        }
        KeyCode::Enter => match app.focus {
            Focus::Input => app.open_file_picker(PickerTarget::Input),
            Focus::Output => app.open_file_picker(PickerTarget::Output),
            _ => {
                app.focus = next_focus(&app.focus);
            }
        },
        KeyCode::Char(c) => {
            app.status.clear();
            app.focused_field_mut().push(c);
        }
        KeyCode::Backspace => {
            app.focused_field_mut().pop();
        }
        _ => {}
    }
    Ok(())
}

fn handle_processing_screen(app: &mut App, code: KeyCode) -> io::Result<()> {
    if code == KeyCode::Esc {
        if let Screen::Processing(ref state) = app.screen {
            state.cancelled.store(true, Ordering::Relaxed);
            app.status = String::from("Cancelling...");
        }
    }
    Ok(())
}

fn handle_done_screen(app: &mut App, code: KeyCode) -> io::Result<()> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('n') => {
            app.screen = Screen::Input;
            app.status.clear();
        }
        _ => {}
    }
    Ok(())
}

fn handle_file_picker(app: &mut App, code: KeyCode) -> io::Result<()> {
    let picker = app.file_picker.as_mut().unwrap();
    let visible = picker_visible_height();
    match code {
        KeyCode::Esc => {
            app.file_picker = None;
            app.picker_for = None;
        }
        KeyCode::Up => {
            if picker.selected > 0 {
                picker.selected -= 1;
                if picker.selected < picker.scroll {
                    picker.scroll = picker.selected;
                }
            }
        }
        KeyCode::Down => {
            if picker.selected + 1 < picker.entries.len() {
                picker.selected += 1;
                if picker.selected >= picker.scroll + visible {
                    picker.scroll = picker.selected.saturating_sub(visible - 1);
                }
            }
        }
        KeyCode::Enter => {
            let entry = &picker.entries[picker.selected];
            let path = picker.current_dir.join(&entry.name);
            if entry.name == ".." {
                if let Some(parent) = picker.current_dir.parent() {
                    picker.current_dir = parent.to_path_buf();
                    refresh_picker(picker);
                }
            } else if entry.is_dir {
                picker.current_dir = path;
                refresh_picker(picker);
            } else {
                // File selected
                let path_str = path.to_string_lossy().to_string();
                if let Some(target) = app.picker_for {
                    match target {
                        PickerTarget::Input => app.input_path = path_str,
                        PickerTarget::Output => app.output_path = path_str,
                    }
                }
                app.file_picker = None;
                app.picker_for = None;
            }
        }
        KeyCode::Backspace => {
            if let Some(parent) = picker.current_dir.parent() {
                picker.current_dir = parent.to_path_buf();
                refresh_picker(picker);
            }
        }
        _ => {}
    }
    Ok(())
}

impl App {
    fn open_file_picker(&mut self, target: PickerTarget) {
        let start_path = match target {
            PickerTarget::Input if !self.input_path.is_empty() => {
                let p = PathBuf::from(&self.input_path);
                if p.is_dir() {
                    p
                } else {
                    p.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
                }
            }
            PickerTarget::Output if !self.output_path.is_empty() => {
                let p = PathBuf::from(&self.output_path);
                if p.is_dir() {
                    p
                } else {
                    p.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
                }
            }
            _ => PathBuf::from("."),
        };

        let current_dir = if start_path.is_absolute() {
            start_path
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(&start_path)
        };

        let mut picker = FilePicker {
            current_dir,
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
        };
        refresh_picker(&mut picker);
        self.file_picker = Some(picker);
        self.picker_for = Some(target);
    }

    fn start_extraction(&mut self) -> io::Result<()> {
        self.status.clear();

        let domain = self.domain.trim().to_string();
        if domain.is_empty() {
            self.status = String::from("Error: Domain is required");
            return Ok(());
        }

        let input_path = PathBuf::from(self.input_path.trim());
        if !input_path.exists() {
            self.status = format!("Error: Input file not found: {}", input_path.display());
            return Ok(());
        }

        let divider = self.divider.chars().next().unwrap_or(':');
        let threads: usize = self.threads.trim().parse().unwrap_or(4).max(1).min(64);

        let total = extractor::count_lines(&input_path)?;
        if total == 0 {
            self.status = String::from("Error: Input file is empty");
            return Ok(());
        }

        let progress = Arc::new(ExtractProgress::new(total));
        let cancelled = Arc::new(AtomicBool::new(false));

        // Spawn extraction in background
        let p = Arc::clone(&progress);
        let c = Arc::clone(&cancelled);
        let d = domain.clone();
        let output = PathBuf::from(self.output_path.trim());
        let input = input_path.clone();
        let input_label = input.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| input.display().to_string());

        std::thread::spawn(move || {
            let _ = extractor::extract(&input, &d, divider, threads, &output, p, c);
        });

        self.screen = Screen::Processing(ProcessState {
            domain,
            input_label,
            progress,
            cancelled,
            done: false,
            matched: 0,
            total,
            duration_ms: 0,
        });

        Ok(())
    }

    fn finish_processing(&mut self, matched: usize, total: usize, cancelled: bool) {
        let domain = match &self.screen {
            Screen::Processing(s) => s.domain.clone(),
            _ => return,
        };
        let output = self.output_path.trim().to_string();
        let dur = match &self.screen {
            Screen::Processing(s) => s.duration_ms,
            _ => 0,
        };

        self.screen = Screen::Done(DoneState {
            domain,
            matched,
            total,
            duration_ms: dur,
            output,
            cancelled,
        });
    }
}

// ── Focus Navigation ───────────────────────────────────────────────────

fn next_focus(f: &Focus) -> Focus {
    match f {
        Focus::Domain => Focus::Threads,
        Focus::Threads => Focus::Divider,
        Focus::Divider => Focus::Input,
        Focus::Input => Focus::Output,
        Focus::Output => Focus::Start,
        Focus::Start => Focus::Domain,
    }
}

fn prev_focus(f: &Focus) -> Focus {
    match f {
        Focus::Domain => Focus::Start,
        Focus::Start => Focus::Output,
        Focus::Output => Focus::Input,
        Focus::Input => Focus::Divider,
        Focus::Divider => Focus::Threads,
        Focus::Threads => Focus::Domain,
    }
}

// ── Rendering ──────────────────────────────────────────────────────────

fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // title
            Constraint::Min(10),    // body
            Constraint::Length(1),  // status
            Constraint::Length(1),  // hints
        ])
        .split(area);

    render_title(f, chunks[0]);
    match &app.screen {
        Screen::Input => render_input_form(f, app, chunks[1]),
        Screen::Processing(state) => render_processing(f, state, chunks[1]),
        Screen::Done(state) => render_done(f, state, chunks[1]),
    }
    render_status(f, app, chunks[2]);
    render_hints(f, app, chunks[3]);

    if let Some(ref picker) = app.file_picker {
        render_file_picker(f, picker);
    }

    // Update processing state
    if let Screen::Processing(ref mut state) = app.screen {
        let p = state.progress.processed.load(Ordering::Relaxed);
        let m = state.progress.matched.load(Ordering::Relaxed);
        let t = state.progress.total.load(Ordering::Relaxed);
        state.total = t;
        state.matched = m;
        if p >= t && t > 0 && !state.done {
            state.done = true;
            state.matched = m;
            let cancelled = state.cancelled.load(Ordering::Relaxed);
            app.finish_processing(m, t, cancelled);
        }
    }
}

// ── Title ──────────────────────────────────────────────────────────────

fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new(
        Text::from(vec![
            Line::from(Span::styled(
                " ulpExtractor ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                " Domain credential extractor",
                Style::default().fg(Color::DarkGray),
            )),
        ]),
    )
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)))
    .alignment(Alignment::Center);

    f.render_widget(title, area);
}

// ── Input Form ─────────────────────────────────────────────────────────

fn render_input_form(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // domain
            Constraint::Length(3), // threads + divider
            Constraint::Length(3), // input
            Constraint::Length(3), // output
            Constraint::Length(3), // start button
        ])
        .split(inner_area(area, 4));

    // Domain
    let style = field_style(app, Focus::Domain);
    let domain = Paragraph::new(app.domain.as_str())
        .block(field_block(" Domain ", style))
        .style(style);
    f.render_widget(domain, chunks[0]);
    if matches!(app.focus, Focus::Domain) {
        render_cursor_placeholder(f, chunks[0], &app.domain, "(e.g. fiverr.com)");
    }

    // Threads + Divider side by side
    let row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(chunks[1]);

    let style = field_style(app, Focus::Threads);
    let threads = Paragraph::new(app.threads.as_str())
        .block(field_block(" Threads ", style))
        .style(style);
    f.render_widget(threads, row[0]);
    if matches!(app.focus, Focus::Threads) {
        render_cursor_placeholder(f, row[0], &app.threads, "4");
    }

    let style = field_style(app, Focus::Divider);
    let divider = Paragraph::new(app.divider.as_str())
        .block(field_block(" Divider ", style))
        .style(style);
    f.render_widget(divider, row[1]);
    if matches!(app.focus, Focus::Divider) {
        render_cursor_placeholder(f, row[1], &app.divider, ":");
    }

    // Input file
    let style = field_style(app, Focus::Input);
    let input = Paragraph::new(app.input_path.as_str())
        .block(field_block(" Input File ", style))
        .style(style);
    f.render_widget(input, chunks[2]);
    if matches!(app.focus, Focus::Input) {
        render_cursor_placeholder(f, chunks[2], &app.input_path, "(press Enter to browse)");
    }

    // Output file
    let style = field_style(app, Focus::Output);
    let output = Paragraph::new(app.output_path.as_str())
        .block(field_block(" Output File ", style))
        .style(style);
    f.render_widget(output, chunks[3]);
    if matches!(app.focus, Focus::Output) {
        render_cursor_placeholder(f, chunks[3], &app.output_path, "(press Enter to browse)");
    }

    // Start button
    let style = if matches!(app.focus, Focus::Start) {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::Cyan)
    };
    let start = Paragraph::new("  Start Extraction  ")
        .block(Block::default().borders(Borders::ALL).border_style(style))
        .style(style)
        .alignment(Alignment::Center);
    f.render_widget(start, chunks[4]);
}

// ── Processing Screen ──────────────────────────────────────────────────

fn render_processing(f: &mut Frame, state: &ProcessState, area: Rect) {
    let p = state.progress.processed.load(Ordering::Relaxed);
    let m = state.progress.matched.load(Ordering::Relaxed);
    let t = state.progress.total.load(Ordering::Relaxed);
    let cancelled = state.cancelled.load(Ordering::Relaxed);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // info
            Constraint::Length(3),  // progress bar
            Constraint::Length(5),  // stats
        ])
        .split(inner_area(area, 4));

    let ratio = if t > 0 { p as f64 / t as f64 } else { 0.0 };
    let pct = (ratio * 100.0) as u32;
    let status_label = if cancelled { "Cancelling..." } else { "Extracting..." };

    // Info line
    let info = Paragraph::new(format!(
        " Domain: {}   Input: {}",
        state.domain, state.input_label
    ))
    .block(Block::default().title(status_label).borders(Borders::ALL).border_style(Style::default().fg(Color::Yellow)))
    .style(Style::default().fg(Color::White));
    f.render_widget(info, chunks[0]);

    // Progress bar
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .ratio(ratio.clamp(0.0, 1.0));
    f.render_widget(gauge, chunks[1]);

    // Percentage overlay on bar
    let pct_text = Paragraph::new(format!(" {}% ", pct))
        .style(Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD));
    let pct_area = centered_rect(chunks[1], 8, 1);
    f.render_widget(pct_text, pct_area);

    // Stats
    let stats = vec![
        Line::from(Span::raw(format!(
            " Lines:    {} / {}",
            format_num(p as u64),
            format_num(t as u64)
        ))),
        Line::from(Span::raw(format!(
            " Matches:  {}",
            format_num(m as u64)
        ))),
    ];
    let stats_p = Paragraph::new(stats)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .style(Style::default().fg(Color::White));
    f.render_widget(stats_p, chunks[2]);
}

// ── Done Screen ────────────────────────────────────────────────────────

fn render_done(f: &mut Frame, state: &DoneState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(7),
            Constraint::Length(3),
        ])
        .split(inner_area(area, 4));

    let icon = if state.cancelled { "✗" } else { "✓" };
    let title = if state.cancelled { "Cancelled" } else { "Complete" };
    let color = if state.cancelled { Color::Yellow } else { Color::Green };

    let res = vec![
        Line::from(Span::styled(
            format!(" {} Extraction {}", icon, title),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::raw(format!(" Domain:      {}", state.domain))),
        Line::from(Span::raw(format!(" Lines read:  {}", format_num(state.total as u64)))),
        Line::from(Span::raw(format!(" Matches:     {}", format_num(state.matched as u64)))),
        Line::from(Span::raw(format!(" Output:      {}", state.output))),
        Line::from(Span::raw(format!(" Duration:    {:.1}s", state.duration_ms as f64 / 1000.0))),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .title(format!(" {} ", title));

    let para = Paragraph::new(res).block(block).style(Style::default().fg(Color::White));
    f.render_widget(para, chunks[0].union(chunks[1]));
}

// ── File Picker ────────────────────────────────────────────────────────

fn render_file_picker(f: &mut Frame, picker: &FilePicker) {
    let popup_area = centered_rect(f.area(), f.area().width.min(60), f.area().height.min(18));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // path
            Constraint::Min(5),      // entries
            Constraint::Length(1),   // hints
        ])
        .split(popup_area);

    // Current path
    let path_text = picker.current_dir.to_string_lossy().to_string();
    let path_p = Paragraph::new(Text::from(path_text).style(Style::default().fg(Color::Cyan)))
        .block(Block::default().borders(Borders::ALL).title(" Browse "))
        .wrap(Wrap { trim: false });
    f.render_widget(path_p, chunks[0]);

    // Entries
    let visible = (chunks[1].height as usize).saturating_sub(2);
    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .enumerate()
        .skip(picker.scroll)
        .take(visible)
        .map(|(i, entry)| {
            let prefix = if entry.name == ".." {
                "  ../"
            } else if entry.is_dir {
                "  📁 "
            } else {
                "  📄 "
            };
            let style = if i == picker.selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Span::styled(
                format!("{}{}", prefix, entry.name),
                style,
            ))
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL));
    f.render_widget(list, chunks[1]);

    // Hints
    let hints = Paragraph::new(" Enter: Select   Esc: Cancel   Backspace: Up   ↑↓: Navigate ")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hints, chunks[2]);
}

// ── Status Line ────────────────────────────────────────────────────────

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let style = if app.status.starts_with("Error") {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let p = Paragraph::new(Text::from(app.status.as_str()).style(style));
    f.render_widget(p, area);
}

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let hints = match &app.screen {
        Screen::Input => " Tab: Next Field   Enter: Browse/Start   Esc: Quit",
        Screen::Processing(_) => " Esc: Cancel",
        Screen::Done(_) => " N: New Extraction   Q: Quit",
    };
    let p = Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}

// ── Helpers ────────────────────────────────────────────────────────────

fn inner_area(area: Rect, margin: u16) -> Rect {
    Rect {
        x: area.x + margin,
        y: area.y,
        width: area.width.saturating_sub(margin * 2),
        height: area.height,
    }
}

fn field_block(title: &str, style: Style) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(style)
        .title(Span::styled(title, style))
}

fn field_style(app: &App, focus: Focus) -> Style {
    if app.focus == focus {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    }
}

fn render_cursor_placeholder(f: &mut Frame, area: Rect, value: &str, placeholder: &str) {
    if !value.is_empty() {
        return;
    }
    let cursor_area = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: 1,
    };
    let p = Paragraph::new(Span::styled(placeholder, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, cursor_area);
}

fn centered_rect(r: Rect, width: u16, height: u16) -> Rect {
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}

fn format_num(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn picker_visible_height() -> usize {
    12
}

fn refresh_picker(picker: &mut FilePicker) {
    picker.entries.clear();
    picker.entries.push(FsEntry {
        name: String::from(".."),
        is_dir: true,
    });

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    if let Ok(read) = std::fs::read_dir(&picker.current_dir) {
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                dirs.push(FsEntry { name, is_dir: true });
            } else {
                files.push(FsEntry { name, is_dir: false });
            }
        }
    }

    dirs.sort_by(|a, b| a.name.cmp(&b.name));
    files.sort_by(|a, b| a.name.cmp(&b.name));
    picker.entries.extend(dirs);
    picker.entries.extend(files);

    picker.selected = picker.selected.min(picker.entries.len().saturating_sub(1));
    picker.scroll = picker.scroll.min(picker.selected);
}
