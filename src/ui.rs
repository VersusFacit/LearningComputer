use std::env;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDate};
use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::{Frame, Terminal};

use crate::controller::{Controller, DailyEntry, DetailMode, Screen, Selected};
use crate::error::AppError;
use crate::loader::{read_snapshot, ReadOutcome, SourceState};

const DEFAULT_TASKS_PATH: &str = "data/tasks.yaml";
const REFRESH_INTERVAL: Duration = Duration::from_millis(750);
const PAGE_STEP: usize = 8;

const fn gundam_background() -> Color {
    Color::Rgb(41, 44, 51)
}

const fn gundam_panel() -> Color {
    Color::Rgb(60, 64, 72)
}

const fn gundam_selection() -> Color {
    Color::Rgb(36, 90, 170)
}

const fn gundam_text() -> Color {
    Color::Rgb(241, 238, 230)
}

const fn gundam_muted() -> Color {
    Color::Rgb(188, 192, 199)
}

const fn gundam_blue() -> Color {
    Color::Rgb(57, 109, 193)
}

const fn gundam_yellow() -> Color {
    Color::Rgb(242, 196, 55)
}

const fn gundam_red() -> Color {
    Color::Rgb(214, 67, 57)
}

const fn gundam_green() -> Color {
    Color::Rgb(128, 201, 132)
}

const fn gundam_border() -> Color {
    Color::Rgb(112, 121, 139)
}

fn panel_block<'a>(title: &'a str) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().bg(gundam_panel()).fg(gundam_text()))
        .border_style(Style::default().fg(gundam_border()))
}

pub fn resolve_tasks_path() -> PathBuf {
    env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| env::var_os("LEARNING_COMPUTER_TASKS_FILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TASKS_PATH))
}

pub fn run(path: PathBuf) -> Result<(), AppError> {
    let initial = read_snapshot(&path, None)?;
    let ReadOutcome::Loaded {
        snapshot,
        source_state,
    } = initial
    else {
        unreachable!("initial load without prior state must produce a snapshot");
    };

    let today = Local::now().date_naive();
    let controller = Controller::new(snapshot, today);
    let mut app = App::new(path, controller, source_state);
    let mut terminal = TerminalSession::enter()?;

    loop {
        terminal.draw(|frame| render(frame, &app))?;

        if event::poll(time_until_refresh(app.last_refresh))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && app.handle_key(key)? {
                    break;
                }
            }
        }

        if app.last_refresh.elapsed() >= REFRESH_INTERVAL {
            app.refresh(false)?;
        }
    }

    Ok(())
}

struct App {
    path: PathBuf,
    controller: Controller,
    source_state: SourceState,
    status: Status,
    last_refresh: Instant,
}

struct Status {
    tone: Tone,
    text: String,
}

enum Tone {
    Neutral,
    Success,
    Warning,
}

impl App {
    fn new(path: PathBuf, controller: Controller, source_state: SourceState) -> Self {
        Self {
            path,
            controller,
            source_state,
            status: Status {
                tone: Tone::Success,
                text: format!("loaded {}", timestamp_label()),
            },
            last_refresh: Instant::now(),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool, AppError> {
        match key.code {
            KeyCode::Char('q') => Ok(true),
            KeyCode::Char('0') => {
                self.controller.set_screen(Screen::Top3);
                Ok(false)
            }
            KeyCode::Char('1') => {
                self.controller.set_screen(Screen::P1);
                Ok(false)
            }
            KeyCode::Char('2') => {
                self.controller.set_screen(Screen::P2);
                Ok(false)
            }
            KeyCode::Char('3') => {
                self.controller.set_screen(Screen::P3);
                Ok(false)
            }
            KeyCode::Char('4') => {
                self.controller.set_screen(Screen::Daily);
                Ok(false)
            }
            KeyCode::Char('5') => {
                self.controller.set_screen(Screen::Decisions);
                Ok(false)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.controller.select_next();
                Ok(false)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.controller.select_previous();
                Ok(false)
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.controller.select_first();
                Ok(false)
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.controller.select_last();
                Ok(false)
            }
            KeyCode::PageDown => {
                for _ in 0..PAGE_STEP {
                    self.controller.select_next();
                }
                Ok(false)
            }
            KeyCode::PageUp => {
                for _ in 0..PAGE_STEP {
                    self.controller.select_previous();
                }
                Ok(false)
            }
            KeyCode::Char('d') => {
                self.controller.cycle_detail_mode();
                Ok(false)
            }
            KeyCode::Char('r') => {
                self.refresh(true)?;
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn refresh(&mut self, manual: bool) -> Result<(), AppError> {
        self.last_refresh = Instant::now();

        match read_snapshot(&self.path, Some(&self.source_state))? {
            ReadOutcome::Loaded {
                snapshot,
                source_state,
            } => {
                self.source_state = source_state;
                self.controller.replace_snapshot(snapshot);
                self.status = Status {
                    tone: Tone::Success,
                    text: format!("reloaded {}", timestamp_label()),
                };
            }
            ReadOutcome::Unchanged { source_state } => {
                self.source_state = source_state;
                if manual {
                    self.status = Status {
                        tone: Tone::Neutral,
                        text: format!("no changes {}", timestamp_label()),
                    };
                }
            }
            ReadOutcome::Rejected {
                error,
                source_state,
            } => {
                self.source_state = source_state;
                self.status = Status {
                    tone: Tone::Warning,
                    text: format!("reload rejected: {error}"),
                };
            }
        }

        Ok(())
    }
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn enter() -> Result<Self, AppError> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    fn draw(&mut self, frame_fn: impl FnOnce(&mut Frame<'_>)) -> Result<(), AppError> {
        self.terminal.draw(frame_fn)?;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            cursor::Show
        );
        let _ = self.terminal.show_cursor();
    }
}

fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(gundam_background()).fg(gundam_text())),
        area,
    );

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    render_header(frame, layout[0], app);
    render_tabs(frame, layout[1], app);
    render_content(frame, layout[2], app);
    render_footer(frame, layout[3], app);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let status_style = match app.status.tone {
        Tone::Neutral => Style::default().fg(gundam_muted()),
        Tone::Success => Style::default().fg(gundam_green()),
        Tone::Warning => Style::default().fg(gundam_red()),
    };

    let path_label = display_path(&app.path);
    let line = Line::from(vec![
        Span::styled(
            " LearningComputer ",
            Style::default()
                .fg(gundam_text())
                .bg(gundam_blue())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(path_label, Style::default().fg(gundam_yellow())),
        Span::raw("  captured "),
        Span::styled(
            app.controller.captured_on().to_string(),
            Style::default().fg(gundam_blue()),
        ),
        Span::raw("  "),
        Span::styled(
            app.status.text.as_str(),
            status_style.add_modifier(Modifier::BOLD),
        ),
    ]);

    let header = Paragraph::new(line)
        .style(Style::default().bg(gundam_background()).fg(gundam_text()))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .style(Style::default().bg(gundam_background()).fg(gundam_text()))
                .border_style(Style::default().fg(gundam_border())),
        );
    frame.render_widget(header, area);
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let titles = ["0 Top3", "1 P1", "2 P2", "3 P3", "4 Daily", "5 Decisions"]
        .into_iter()
        .map(Line::from)
        .collect::<Vec<_>>();

    let selected = match app.controller.screen {
        Screen::Top3 => 0,
        Screen::P1 => 1,
        Screen::P2 => 2,
        Screen::P3 => 3,
        Screen::Daily => 4,
        Screen::Decisions => 5,
    };

    let tabs = Tabs::new(titles)
        .select(selected)
        .block(panel_block(" Views "))
        .style(Style::default().fg(gundam_muted()).bg(gundam_panel()))
        .highlight_style(
            Style::default()
                .fg(gundam_text())
                .bg(gundam_blue())
                .add_modifier(Modifier::BOLD),
        )
        .divider(" ");

    frame.render_widget(tabs, area);
}

fn render_content(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let segments = if app.controller.detail_mode == DetailMode::Closed {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(area)
    };

    render_list(frame, segments[0], app);

    if app.controller.detail_mode != DetailMode::Closed {
        render_detail(frame, segments[1], app);
    }
}

fn render_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items: Vec<ListItem<'_>> = match app.controller.screen {
        Screen::Top3 => app
            .controller
            .top_three()
            .map(|task| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:>4} ", task.rank),
                        Style::default().fg(gundam_yellow()),
                    ),
                    Span::styled(task.title.as_str(), Style::default().fg(gundam_text())),
                    Span::raw(" "),
                    Span::styled(
                        label_for_task_status(&task.status),
                        Style::default().fg(gundam_blue()),
                    ),
                ]))
            })
            .collect(),
        Screen::P1 => app
            .controller
            .p1()
            .map(|task| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:>4} ", task.rank),
                        Style::default().fg(gundam_yellow()),
                    ),
                    Span::styled(task.title.as_str(), Style::default().fg(gundam_text())),
                    Span::raw(" "),
                    Span::styled(
                        label_for_task_status(&task.status),
                        Style::default().fg(gundam_blue()),
                    ),
                ]))
            })
            .collect(),
        Screen::P2 => app
            .controller
            .p2()
            .iter()
            .enumerate()
            .map(|(index, task)| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:>2} ", index + 1),
                        Style::default().fg(gundam_yellow()),
                    ),
                    Span::styled(task.title.as_str(), Style::default().fg(gundam_text())),
                ]))
            })
            .collect(),
        Screen::P3 => app
            .controller
            .p3()
            .iter()
            .enumerate()
            .map(|(index, task)| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:>2} ", index + 1),
                        Style::default().fg(gundam_yellow()),
                    ),
                    Span::styled(task.title.as_str(), Style::default().fg(gundam_text())),
                    Span::raw(" "),
                    Span::styled(
                        label_for_task_status(&task.status),
                        Style::default().fg(gundam_blue()),
                    ),
                ]))
            })
            .collect(),
        Screen::Daily => app
            .controller
            .daily()
            .map(|entry| daily_list_item(entry))
            .collect(),
        Screen::Decisions => app
            .controller
            .decisions()
            .iter()
            .map(decision_list_item)
            .collect(),
    };

    let title = match app.controller.screen {
        Screen::Top3 => " Today's Top 3 ",
        Screen::P1 => " P1 ",
        Screen::P2 => " P2 ",
        Screen::P3 => " P3 ",
        Screen::Daily => " Daily ",
        Screen::Decisions => " Decisions ",
    };

    let list = List::new(items)
        .block(panel_block(title))
        .style(Style::default().bg(gundam_panel()).fg(gundam_text()))
        .highlight_symbol(">> ")
        .highlight_style(
            Style::default()
                .bg(gundam_selection())
                .fg(gundam_text())
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default().with_selected(Some(app.controller.selection()));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_detail(frame: &mut Frame<'_>, area: Rect, app: &App) {
    match app.controller.detail_mode {
        DetailMode::Closed => {}
        DetailMode::Item => {
            let detail = Paragraph::new(selected_detail_text(app))
                .style(Style::default().bg(gundam_panel()).fg(gundam_text()))
                .block(panel_block(" Detail: Item "))
                .wrap(Wrap { trim: false });

            frame.render_widget(detail, area);
        }
    }
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let detail = match app.controller.detail_mode {
        DetailMode::Closed => "closed",
        DetailMode::Item => "item",
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" q ", Style::default().fg(gundam_text()).bg(gundam_red())),
        Span::raw("quit  "),
        Span::styled(" r ", Style::default().fg(gundam_text()).bg(gundam_green())),
        Span::raw("reload  "),
        Span::styled(
            " d ",
            Style::default().fg(gundam_text()).bg(gundam_yellow()),
        ),
        Span::raw(format!("detail:{detail}  ")),
        Span::styled(
            " j/k ",
            Style::default().fg(gundam_text()).bg(gundam_blue()),
        ),
        Span::raw("move  "),
        Span::styled(
            " g/G ",
            Style::default().fg(gundam_text()).bg(gundam_blue()),
        ),
        Span::raw("first/last  "),
        Span::raw("PgUp/PgDn move faster"),
    ]))
    .style(Style::default().bg(gundam_background()).fg(gundam_text()))
    .block(
        Block::default()
            .borders(Borders::TOP)
            .style(Style::default().bg(gundam_background()).fg(gundam_text()))
            .border_style(Style::default().fg(gundam_border())),
    );

    frame.render_widget(footer, area);
}

fn selected_detail_text(app: &App) -> Text<'static> {
    let Some(selected) = app.controller.selected() else {
        return Text::from("No item selected.");
    };

    match selected {
        Selected::P1(task) => task_text(
            task.title.as_str(),
            task.raw_text.as_str(),
            &task.links,
            &task.notes,
            Some(format!("rank {}", task.rank)),
            task.completed_at,
        ),
        Selected::P2(task) => task_text(
            task.title.as_str(),
            task.raw_text.as_str(),
            &task.links,
            &task.notes,
            Some(format!("source {}", task.source_order)),
            task.completed_at,
        ),
        Selected::P3(task) => task_text(
            task.title.as_str(),
            task.raw_text.as_str(),
            &task.links,
            &task.notes,
            Some(format!("source {}", task.source_order)),
            task.completed_at,
        ),
        Selected::Daily(entry) => daily_text(entry),
        Selected::Decision(decision) => decision_text(decision),
    }
}

fn decision_text(decision: &crate::model::Decision) -> Text<'static> {
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        format!("{} ({})", decision.title, decision.date),
        Style::default()
            .fg(gundam_blue())
            .add_modifier(Modifier::BOLD),
    )));

    if !decision.settings.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "settings",
            Style::default()
                .fg(gundam_yellow())
                .add_modifier(Modifier::BOLD),
        )));
        for (key, value) in &decision.settings {
            lines.push(Line::from(format!(
                "  {} = {}",
                key,
                format_yaml_value(value)
            )));
        }
    }

    if !decision.summary.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::from(Span::styled(
            "summary",
            Style::default()
                .fg(gundam_yellow())
                .add_modifier(Modifier::BOLD),
        )));
        for item in &decision.summary {
            lines.push(Line::from(format!("  - {item}")));
        }
    }

    if !decision.startup_flow_notes.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::from(Span::styled(
            "notes",
            Style::default()
                .fg(gundam_yellow())
                .add_modifier(Modifier::BOLD),
        )));
        for item in &decision.startup_flow_notes {
            lines.push(Line::from(format!("  - {item}")));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from("No settings or notes."));
    }

    Text::from(lines)
}

fn task_text(
    title: &str,
    raw_text: &str,
    links: &[String],
    notes: &[String],
    meta: Option<String>,
    completed_at: Option<NaiveDate>,
) -> Text<'static> {
    let mut lines = vec![Line::from(Span::styled(
        title.to_string(),
        Style::default()
            .fg(gundam_blue())
            .add_modifier(Modifier::BOLD),
    ))];

    if let Some(meta) = meta {
        lines.push(Line::from(meta));
    }

    if let Some(completed_at) = completed_at {
        lines.push(Line::from(format!("completed: {completed_at}")));
    }

    lines.push(Line::default());
    lines.push(Line::from("raw:"));
    lines.push(Line::from(format!("  {raw_text}")));

    if !notes.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from("notes:"));
        for note in notes {
            lines.push(Line::from(format!("  - {note}")));
        }
    }

    if !links.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from("links:"));
        for link in links {
            lines.push(Line::from(format!("  - {link}")));
        }
    }

    Text::from(lines)
}

fn daily_text(entry: DailyEntry<'_>) -> Text<'static> {
    let mut lines = vec![
        Line::from(Span::styled(
            entry.task.title.clone(),
            Style::default()
                .fg(gundam_blue())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("bucket: {:?}", entry.bucket).to_lowercase()),
        Line::from(format!(
            "last hit: {}",
            entry
                .last_hit
                .map(|date| date.to_string())
                .unwrap_or_else(|| "never".to_string())
        )),
        Line::from(format!("stale: {}", if entry.stale { "yes" } else { "no" })),
        Line::default(),
        Line::from("raw:"),
        Line::from(format!("  {}", entry.task.raw_text)),
    ];

    if !entry.task.notes.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from("notes:"));
        for note in &entry.task.notes {
            lines.push(Line::from(format!("  - {note}")));
        }
    }

    if !entry.task.links.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from("links:"));
        for link in &entry.task.links {
            lines.push(Line::from(format!("  - {link}")));
        }
    }

    Text::from(lines)
}

fn daily_list_item(entry: DailyEntry<'_>) -> ListItem<'_> {
    let bucket = match entry.bucket {
        crate::controller::DailyBucket::Active => "A",
        crate::controller::DailyBucket::Later => "L",
    };

    let stale_style = if entry.stale {
        Style::default()
            .fg(gundam_red())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(gundam_green())
    };

    ListItem::new(Line::from(vec![
        Span::styled(format!("[{bucket}] "), Style::default().fg(gundam_yellow())),
        Span::styled(
            entry.task.title.as_str(),
            Style::default().fg(gundam_text()),
        ),
        Span::raw(" "),
        Span::styled(
            entry
                .last_hit
                .map(|date| date.to_string())
                .unwrap_or_else(|| "never".to_string()),
            Style::default().fg(gundam_blue()),
        ),
        Span::raw(" "),
        Span::styled(if entry.stale { "STALE" } else { "fresh" }, stale_style),
    ]))
}

fn decision_list_item(decision: &crate::model::Decision) -> ListItem<'_> {
    ListItem::new(Line::from(vec![
        Span::styled(
            format!("{} ", decision.date),
            Style::default().fg(gundam_yellow()),
        ),
        Span::styled(decision.title.as_str(), Style::default().fg(gundam_text())),
    ]))
}

fn format_yaml_value(value: &serde_norway::Value) -> String {
    match value {
        serde_norway::Value::Bool(value) => value.to_string(),
        serde_norway::Value::Number(value) => value.to_string(),
        serde_norway::Value::String(value) => value.clone(),
        other => format!("{other:?}"),
    }
}

fn label_for_task_status(status: &crate::model::TaskStatus) -> &'static str {
    match status {
        crate::model::TaskStatus::Todo => "todo",
        crate::model::TaskStatus::Done => "done",
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn timestamp_label() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

fn time_until_refresh(last_refresh: Instant) -> Duration {
    REFRESH_INTERVAL.saturating_sub(last_refresh.elapsed())
}
