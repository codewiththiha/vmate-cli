//! Connect-mode status UI.
//!
//! Renders a compact status block on an alternate screen and translates keys
//! into [`UserCommand`]s for the connect service.

use crate::ui::term::TuiGuard;
use anyhow::Result;
use async_trait::async_trait;
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, enable_raw_mode};
use std::collections::VecDeque;
use std::io::Write;
use std::time::{Duration, Instant};
use vmate_core::connect::{ConnectHost, ConnectionStatus, UserCommand};

/// The connect-mode terminal host.
pub struct ConnectTui {
    connected: bool,
    connected_since: Option<Instant>,
    verbose: bool,
    no_interactive: bool,
    filter: String,
    status: ConnectionStatus,
    log_buffer: VecDeque<String>,
    _guard: TuiGuard,
}

impl ConnectTui {
    pub fn new(no_interactive: bool, filter: String) -> Result<Self> {
        enable_raw_mode()?;
        execute!(std::io::stdout(), EnterAlternateScreen)?;
        Ok(Self {
            connected: false,
            connected_since: None,
            verbose: false,
            no_interactive,
            filter,
            status: ConnectionStatus {
                connected: false,
                candidate: None,
                message: String::new(),
                filter: String::new(),
            },
            log_buffer: VecDeque::new(),
            _guard: TuiGuard,
        })
    }

    fn render(&self) -> Result<()> {
        let mut out = std::io::stdout();
        execute!(out, MoveTo(0, 0), Clear(ClearType::All))?;
        let mut w = out.lock();

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

        if self.connected {
            let uptime = self
                .connected_since
                .map(|t| format_duration(t.elapsed()))
                .unwrap_or_else(|| "00:00:00".to_string());
            writeln!(w, "Connected: {country}")?;
            writeln!(w, "Config:    {path}")?;
            writeln!(w, "Uptime:    {uptime}")?;
        } else {
            writeln!(w, "Connecting: {country}")?;
            writeln!(w, "Config:     {path}")?;
        }
        writeln!(w, "Filter:    {}", self.filter)?;
        writeln!(w)?;
        writeln!(w, "{}", self.status.message)?;
        writeln!(w)?;
        writeln!(
            w,
            "[n] next  [r] reconnect  [c] copy path  [v] verbose  [q] quit"
        )?;

        if self.verbose {
            writeln!(w)?;
            writeln!(w, "--- OpenVPN output ---")?;
            for line in &self.log_buffer {
                writeln!(w, "{line}")?;
            }
        }
        w.flush()?;
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
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('n') => Some(UserCommand::Next),
                KeyCode::Char('r') => Some(UserCommand::Reconnect),
                KeyCode::Char('v') => Some(UserCommand::ToggleVerbose),
                KeyCode::Char('?') => Some(UserCommand::Help),
                KeyCode::Char('q') => Some(UserCommand::Quit),
                KeyCode::Esc => Some(UserCommand::Quit),
                KeyCode::Char('c') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(UserCommand::CopyPath)
                }
                _ => None,
            },
            _ => None,
        }
    }
}

#[async_trait]
impl ConnectHost for ConnectTui {
    async fn status(&mut self, status: &ConnectionStatus) -> Result<()> {
        self.status = status.clone();
        self.connected = status.connected;
        if status.connected && self.connected_since.is_none() {
            self.connected_since = Some(Instant::now());
        } else if !status.connected {
            self.connected_since = None;
        }
        self.render()
    }

    async fn notify(&mut self, message: &str) -> Result<()> {
        self.status.message = message.to_string();
        self.render()
    }

    async fn log(&mut self, line: &str) -> Result<()> {
        self.log_buffer.push_back(line.to_string());
        if self.log_buffer.len() > 200 {
            self.log_buffer.pop_front();
        }
        self.render()
    }

    async fn copy(&mut self, text: &str) -> Result<()> {
        match crate::ui::clipboard::copy_to_clipboard(text) {
            Ok(method) => {
                self.status.message = format!("Copied: {text} ({method})");
            }
            Err(err) => {
                self.status.message = format!("copy failed: {err}");
            }
        }
        self.render()
    }

    async fn poll_command(&mut self) -> Option<UserCommand> {
        // Re-render first so the uptime counter keeps ticking while idle.
        let _ = self.render();
        self.read_key()
    }

    async fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}

fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}
