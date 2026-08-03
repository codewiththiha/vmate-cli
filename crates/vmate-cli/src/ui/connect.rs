//! Connect-mode status UI, rendered with ratatui (same toolkit as `recent`).
//!
//! Using ratatui avoids the raw-mode newline bug that plagues hand-rolled
//! crossterm rendering: ratatui owns cursor positioning, so every line starts
//! at column 0. `Ctrl+C` quits (in raw mode it arrives as a control-modified
//! `c`, not SIGINT).

use crate::ui::term::TuiGuard;
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use std::collections::VecDeque;
use std::io::Stdout;
use std::time::{Duration, Instant};
use vmate_core::connect::{ConnectHost, ConnectionStatus, UserCommand};

type Term = Terminal<CrosstermBackend<Stdout>>;

/// How long transient messages (e.g. `Copied: ...`, help text) stay visible
/// before reverting to the connected status.
const MESSAGE_TTL: Duration = Duration::from_secs(3);

/// Whether a transient overlay came from the connect status (the fading
/// "Connected successfully to X" confirmation) or from a user-facing notice
/// (`notify`/`copy`, e.g. "removed ... from recent list"). A confirmation is
/// stale the moment we leave the connected state; a notice keeps riding out
/// its TTL across a candidate switch.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TransientKind {
    Connected,
    Notice,
}

/// A transient overlay message with the instant it was shown and its kind. It
/// overrides `status.message` until it fades after `MESSAGE_TTL`, then the
/// persistent status message (or nothing, once connected) is shown instead.
/// Keeping the text here lets a notice survive the next `status()` call.
struct Transient {
    kind: TransientKind,
    text: String,
    shown_at: Instant,
}

/// The connect-mode terminal host.
pub struct ConnectTui {
    term: Term,
    connected_since: Option<Instant>,
    transient: Option<Transient>,
    verbose: bool,
    no_interactive: bool,
    filter: String,
    status: ConnectionStatus,
    log: VecDeque<String>,
    _guard: TuiGuard,
}

impl ConnectTui {
    pub fn new(no_interactive: bool, filter: String, verbose: bool) -> Result<Self> {
        enable_raw_mode()?;
        // Held immediately so the terminal is restored even if a later step
        // fails; moved into the struct once fully constructed.
        let guard = TuiGuard;
        execute!(
            std::io::stdout(),
            EnterAlternateScreen,
            event::EnableMouseCapture
        )?;
        let term = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
        Ok(Self {
            term,
            connected_since: None,
            transient: None,
            verbose,
            no_interactive,
            filter,
            status: ConnectionStatus {
                connected: false,
                candidate: None,
                message: String::new(),
                filter: String::new(),
            },
            log: VecDeque::new(),
            _guard: guard,
        })
    }

    fn draw(&mut self) -> Result<()> {
        let uptime = self
            .connected_since
            .map(|t| format_duration(t.elapsed()))
            .unwrap_or_else(|| "00:00:00".to_string());
        let country = self
            .status
            .candidate
            .as_ref()
            .map(|c| c.country.as_str())
            .unwrap_or("-");
        let file_name = self
            .status
            .candidate
            .as_ref()
            .map(|c| c.file_name())
            .unwrap_or_else(|| "-".to_string());
        let (state, color) = if self.status.connected {
            ("Connected", Color::Green)
        } else {
            ("Connecting", Color::Yellow)
        };
        let filter = self.filter.clone();
        // Expire a stale overlay, then render: a fresh transient overrides the
        // status message; once connected the confirmation fades to empty;
        // connecting/reconnecting statuses stay persistent.
        if let Some(t) = &self.transient {
            if t.shown_at.elapsed() >= MESSAGE_TTL {
                self.transient = None;
            }
        }
        let message = match &self.transient {
            Some(t) => t.text.clone(),
            None if self.status.connected => String::new(),
            None => self.status.message.clone(),
        };
        let verbose = self.verbose;
        let log_text = if verbose {
            self.log.iter().cloned().collect::<Vec<_>>().join("\n")
        } else {
            String::new()
        };

        let body = vec![
            Line::from(Span::styled(
                format!("{state}: {country}"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(format!("Config : {file_name}")),
            Line::from(format!("Uptime : {uptime}")),
            Line::from(format!("Filter : {filter}")),
            Line::from(""),
            Line::from(Span::styled(
                message,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::DIM),
            )),
        ];

        self.term.draw(|f| {
            let area = f.area();
            let chunks = Layout::vertical([
                Constraint::Min(3),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(area);

            f.render_widget(Paragraph::new(body), chunks[0]);

            if verbose {
                let log = Paragraph::new(log_text)
                    .style(Style::default().fg(Color::DarkGray))
                    .wrap(Wrap { trim: false });
                f.render_widget(log, chunks[1]);
            }

            let footer = Paragraph::new(Line::from(vec![
                Span::styled(
                    "[n]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" next  "),
                Span::styled(
                    "[r]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" reconnect  "),
                Span::styled(
                    "[c]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" copy  "),
                Span::styled(
                    "[v]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" verbose  "),
                Span::styled(
                    "[q]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" quit"),
            ]))
            .style(Style::default().add_modifier(Modifier::REVERSED));
            f.render_widget(footer, chunks[2]);
        })?;
        Ok(())
    }

    fn read_key(&mut self) -> Option<UserCommand> {
        if self.no_interactive {
            return None;
        }
        if !event::poll(Duration::from_millis(200)).ok()? {
            return None;
        }
        match event::read().ok()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                // Ctrl+C quits (raw mode swallows SIGINT).
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(UserCommand::Quit)
                }
                KeyCode::Char('n') => Some(UserCommand::Next),
                KeyCode::Char('r') => Some(UserCommand::Reconnect),
                KeyCode::Char('v') => {
                    self.verbose = !self.verbose;
                    Some(UserCommand::ToggleVerbose)
                }
                KeyCode::Char('?') => Some(UserCommand::Help),
                KeyCode::Char('q') | KeyCode::Esc => Some(UserCommand::Quit),
                KeyCode::Char('c') => Some(UserCommand::CopyPath),
                _ => None,
            },
            _ => None,
        }
    }
}

#[async_trait]
impl ConnectHost for ConnectTui {
    async fn status(&mut self, s: &ConnectionStatus) -> Result<()> {
        if s.connected && self.connected_since.is_none() {
            self.connected_since = Some(Instant::now());
        } else if !s.connected {
            self.connected_since = None;
        }
        let was_connected = self.status.connected;
        self.status = s.clone();
        // Once connected, show a fading confirmation. When connecting (or
        // switching), leave any active transient alone so a notice like
        // "removed ... from recent list" keeps showing while the next config
        // connects, then fades to reveal the connecting status.
        if s.connected {
            let country = s
                .candidate
                .as_ref()
                .map(|c| c.country.as_str())
                .unwrap_or("-");
            self.transient = Some(Transient {
                kind: TransientKind::Connected,
                text: format!("Connected successfully to {country}"),
                shown_at: Instant::now(),
            });
        } else if was_connected {
            // Leaving the connected state (skip to the next config, reconnect,
            // crash): the "Connected successfully to X" confirmation is now
            // stale and would contradict the connecting header — drop it. Real
            // notices survive the switch and keep riding out their TTL.
            if matches!(
                self.transient,
                Some(Transient { kind: TransientKind::Connected, .. })
            ) {
                self.transient = None;
            }
        }
        self.draw()
    }

    async fn notify(&mut self, message: &str) -> Result<()> {
        self.status.message = message.to_string();
        self.transient = Some(Transient {
            kind: TransientKind::Notice,
            text: message.to_string(),
            shown_at: Instant::now(),
        });
        self.draw()
    }

    async fn log(&mut self, line: &str) -> Result<()> {
        self.log.push_back(line.to_string());
        if self.log.len() > 200 {
            self.log.pop_front();
        }
        self.draw()
    }

    async fn copy(&mut self, text: &str) -> Result<()> {
        let message = match crate::ui::clipboard::copy_to_clipboard(text) {
            Ok(method) => format!("Copied: {text} ({method})"),
            Err(err) => format!("copy failed: {err}"),
        };
        self.transient = Some(Transient {
            kind: TransientKind::Notice,
            text: message,
            shown_at: Instant::now(),
        });
        self.draw()
    }

    async fn poll_command(&mut self) -> Option<UserCommand> {
        let _ = self.draw();
        self.read_key()
    }

    async fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}

fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}
