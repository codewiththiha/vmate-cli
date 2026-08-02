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

/// The connect-mode terminal host.
pub struct ConnectTui {
    term: Term,
    connected_since: Option<Instant>,
    /// When the last transient message was shown (`copy`/`notify`, or the
    /// connected confirmation). `None` means the current message is a
    /// persistent service status (connecting/reconnecting/retrying).
    message_since: Option<Instant>,
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
            message_since: None,
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
        let path = self
            .status
            .candidate
            .as_ref()
            .map(|c| c.path.as_str())
            .unwrap_or("-");
        let (state, color) = if self.status.connected {
            ("Connected", Color::Green)
        } else {
            ("Connecting", Color::Yellow)
        };
        let filter = self.filter.clone();
        // Transient messages (Copied/help/connected confirmation) fade after
        // MESSAGE_TTL; connecting/reconnecting statuses stay persistent.
        let message = match self.message_since {
            Some(ts) if ts.elapsed() < MESSAGE_TTL => self.status.message.clone(),
            Some(_) => String::new(),
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
            Line::from(format!("Config : {path}")),
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
        self.status = s.clone();
        // Once connected, the "Connected successfully to {country}" message is
        // redundant with the "Connected: {country}" header, so it fades like
        // other transient messages. Connecting/reconnecting statuses stay up.
        self.message_since = if s.connected {
            Some(Instant::now())
        } else {
            None
        };
        self.draw()
    }

    async fn notify(&mut self, message: &str) -> Result<()> {
        self.status.message = message.to_string();
        self.message_since = Some(Instant::now());
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
        self.status.message = match crate::ui::clipboard::copy_to_clipboard(text) {
            Ok(method) => format!("Copied: {text} ({method})"),
            Err(err) => format!("copy failed: {err}"),
        };
        self.message_since = Some(Instant::now());
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
