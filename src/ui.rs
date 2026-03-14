use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use crate::model::{ProgressSnapshot, RunConfig, ScanOptions, ScanSummary};
use crate::report;
use crate::scanner::{self, ScanEvent};

const MAX_LOG_LINES: usize = 500;

pub fn run_tui(config: RunConfig) -> Result<()> {
    let (tx, rx) = mpsc::channel::<UiEvent>();
    let worker_config = config.clone();

    thread::spawn(move || {
        let start = Instant::now();
        let _ = tx.send(UiEvent::Phase(String::from("Scanning library")));

        let scan_result = scanner::scan_library(
            &worker_config.scan_root,
            &ScanOptions {
                modified_within_days: worker_config.modified_within_days,
            },
            |event| {
                let _ = tx.send(UiEvent::Scan(event));
            },
        );

        match scan_result {
            Ok(result) => {
                let _ = tx.send(UiEvent::Phase(String::from("Writing reports")));
                let report_result = report::write_reports(
                    &worker_config.output_path,
                    &worker_config.error_path,
                    worker_config.output_format,
                    &worker_config.scan_root,
                    &result.scanned_files,
                    &result.errors,
                    &result.summary,
                    &result.supported_format_counts,
                    &result.skipped_format_counts,
                    worker_config.modified_within_days,
                );

                match report_result {
                    Ok(()) => {
                        let _ = tx.send(UiEvent::Completed {
                            elapsed: start.elapsed(),
                            summary: result.summary,
                            supported_format_counts: result.supported_format_counts,
                            skipped_format_counts: result.skipped_format_counts,
                        });
                    }
                    Err(error) => {
                        let _ = tx.send(UiEvent::Fatal(error.to_string()));
                    }
                }
            }
            Err(error) => {
                let _ = tx.send(UiEvent::Fatal(error.to_string()));
            }
        }
    });

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_event_loop(&mut terminal, rx, config);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    rx: Receiver<UiEvent>,
    config: RunConfig,
) -> Result<()> {
    let mut app = AppState::new(config);

    loop {
        while let Ok(event) = rx.try_recv() {
            app.handle_event(event);
        }

        terminal.draw(|frame| draw(frame, &app))?;

        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc if app.done => break,
                        KeyCode::Up => app.scroll_up(),
                        KeyCode::Down => app.scroll_down(),
                        KeyCode::PageUp => app.page_up(),
                        KeyCode::PageDown => app.page_down(),
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(())
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &AppState) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(10),
            Constraint::Length(4),
        ])
        .split(frame.area());

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(areas[1]);

    let header_text = vec![
        Line::from(vec![
            Span::styled(
                "Album Cover Check",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                app.phase_line(),
                Style::default().fg(if app.failed {
                    Color::Red
                } else if app.done {
                    Color::Green
                } else {
                    Color::Yellow
                }),
            ),
        ]),
        Line::from(format!("Root: {}", app.config.scan_root.display())),
        Line::from(format!(
            "Format: {}    Modified filter: {}",
            app.config.output_format.as_str(),
            format_modified_filter(app.config.modified_within_days)
        )),
        Line::from(format!("Output dir: {}", app.config.output_dir.display())),
        Line::from(format!(
            "Config: {}",
            app.config
                .config_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| String::from("[none]"))
        )),
    ];
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title("Session"))
        .wrap(Wrap { trim: true });
    frame.render_widget(header, areas[0]);

    let stats_text = vec![
        Line::from(if app.done {
            Span::styled(
                "Scan Complete",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else if app.failed {
            Span::styled(
                "Scan Failed",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "Scanning",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        }),
        Line::from(""),
        Line::from(format!("Processed: {}", app.summary.processed_audio)),
        Line::from(format!(
            "Supported scanned: {}",
            app.summary.scanned_supported
        )),
        Line::from(format!(
            "Missing embedded cover: {}",
            app.summary.missing_front_cover
        )),
        Line::from(format!("Errors: {}", app.summary.errors)),
        Line::from(format!(
            "Unsupported skipped: {}",
            app.summary.skipped_unsupported
        )),
        Line::from(""),
        Line::from(format!(
            "Supported formats: {}",
            format_extension_counts(&app.supported_format_counts)
        )),
        Line::from(""),
        Line::from(format!(
            "Unsupported formats: {}",
            format_extension_counts(&app.skipped_format_counts)
        )),
        Line::from(""),
        Line::from(if app.done || app.failed {
            String::from("Press q or Esc to close")
        } else {
            String::from("Use Up/Down or PageUp/PageDown to scroll")
        }),
    ];
    let stats = Paragraph::new(stats_text)
        .block(Block::default().borders(Borders::ALL).title("Live Summary"))
        .wrap(Wrap { trim: true });
    frame.render_widget(stats, middle[0]);

    let visible_height = middle[1].height.saturating_sub(2) as usize;
    let total_lines = app.logs.len();
    let start = total_lines.saturating_sub(visible_height + app.log_scroll);
    let end = total_lines.saturating_sub(app.log_scroll);
    let items: Vec<ListItem<'_>> = app
        .logs
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|line| ListItem::new(line.clone()))
        .collect();
    let logs = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Activity")
                .title_bottom("Up/Down to scroll"),
        )
        .style(Style::default().fg(Color::White));
    frame.render_widget(logs, middle[1]);

    let footer_text = if app.done || app.failed {
        vec![
            Line::from(format!("Main report: {}", app.config.output_path.display())),
            Line::from(format!("Error log: {}", app.config.error_path.display())),
            Line::from(format!(
                "Elapsed: {:.1}s    Press q or Esc to close",
                app.elapsed.as_secs_f64()
            )),
        ]
    } else {
        vec![
            Line::from(format!("Main report: {}", app.config.output_path.display())),
            Line::from(format!("Error log: {}", app.config.error_path.display())),
            Line::from("Scanning in progress..."),
        ]
    };
    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL).title("Outputs"))
        .wrap(Wrap { trim: true });
    frame.render_widget(footer, areas[2]);

    if app.failed {
        let footer = Paragraph::new(vec![
            Line::from(format!("Main report: {}", app.config.output_path.display())),
            Line::from(format!("Error log: {}", app.config.error_path.display())),
            Line::from(app.failure_message.clone().unwrap_or_default()),
        ])
        .block(Block::default().borders(Borders::ALL).title("Outputs"))
        .wrap(Wrap { trim: true });
        frame.render_widget(footer, areas[2]);
    }
}

#[derive(Debug)]
enum UiEvent {
    Scan(ScanEvent),
    Phase(String),
    Completed {
        elapsed: Duration,
        summary: ScanSummary,
        supported_format_counts: BTreeMap<String, usize>,
        skipped_format_counts: BTreeMap<String, usize>,
    },
    Fatal(String),
}

struct AppState {
    config: RunConfig,
    logs: VecDeque<String>,
    log_scroll: usize,
    summary: ScanSummary,
    supported_format_counts: BTreeMap<String, usize>,
    skipped_format_counts: BTreeMap<String, usize>,
    phase: String,
    done: bool,
    failed: bool,
    failure_message: Option<String>,
    elapsed: Duration,
}

impl AppState {
    fn new(config: RunConfig) -> Self {
        let mut logs = VecDeque::new();
        logs.push_back(String::from("Preparing scan..."));
        Self {
            config,
            logs,
            log_scroll: 0,
            summary: ScanSummary::default(),
            supported_format_counts: BTreeMap::new(),
            skipped_format_counts: BTreeMap::new(),
            phase: String::from("Preparing"),
            done: false,
            failed: false,
            failure_message: None,
            elapsed: Duration::from_secs(0),
        }
    }

    fn handle_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Scan(scan_event) => match scan_event {
                ScanEvent::Started => {
                    self.phase = String::from("Scanning");
                    self.push_log(format!(
                        "Scanning started at {}",
                        self.config.scan_root.display()
                    ));
                }
                ScanEvent::Progress(snapshot) | ScanEvent::Finished(snapshot) => {
                    self.apply_progress(snapshot);
                }
                ScanEvent::Error(record) => {
                    self.summary.errors = self.summary.errors.max(1);
                    self.push_log(format!(
                        "Error reading {}: {}",
                        record.path.display(),
                        record.message
                    ));
                }
                ScanEvent::UnsupportedAudio { extension, path } => {
                    self.push_log(format!(
                        "Skipped unsupported audio .{}: {}",
                        extension,
                        path.display()
                    ));
                }
            },
            UiEvent::Phase(phase) => {
                self.phase = phase.clone();
                self.push_log(phase);
            }
            UiEvent::Completed {
                elapsed,
                summary,
                supported_format_counts,
                skipped_format_counts,
            } => {
                self.phase = String::from("Done");
                self.elapsed = elapsed;
                self.summary = summary;
                self.supported_format_counts = supported_format_counts;
                self.skipped_format_counts = skipped_format_counts;
                self.done = true;
                self.push_log(String::from("Reports written successfully."));
            }
            UiEvent::Fatal(message) => {
                self.phase = String::from("Failed");
                self.failed = true;
                self.failure_message = Some(message.clone());
                self.push_log(format!("Fatal error: {message}"));
            }
        }
    }

    fn apply_progress(&mut self, snapshot: ProgressSnapshot) {
        self.summary = snapshot.summary;
        self.supported_format_counts = snapshot.supported_format_counts;
        self.skipped_format_counts = snapshot.skipped_format_counts;
    }

    fn push_log(&mut self, line: String) {
        if self.logs.len() == MAX_LOG_LINES {
            self.logs.pop_front();
        }
        self.logs.push_back(line);
    }

    fn phase_line(&self) -> String {
        if self.failed {
            return String::from("Failed");
        }
        if self.done {
            return String::from("Done");
        }
        self.phase.clone()
    }

    fn scroll_up(&mut self) {
        self.log_scroll = self.log_scroll.saturating_add(1);
    }

    fn scroll_down(&mut self) {
        self.log_scroll = self.log_scroll.saturating_sub(1);
    }

    fn page_up(&mut self) {
        self.log_scroll = self.log_scroll.saturating_add(8);
    }

    fn page_down(&mut self) {
        self.log_scroll = self.log_scroll.saturating_sub(8);
    }
}

fn format_extension_counts(counts: &BTreeMap<String, usize>) -> String {
    if counts.is_empty() {
        return String::from("[none]");
    }

    counts
        .iter()
        .map(|(extension, count)| format!(".{extension} x{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_modified_filter(days: Option<u64>) -> String {
    match days {
        Some(days) => format!("last {days} days"),
        None => String::from("none"),
    }
}
