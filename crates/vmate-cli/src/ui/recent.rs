//! Ratatui list for browsing recent successful configs.
//!
//! Supports arrow-key navigation, Enter/`c` to copy a path, mouse click to
//! copy, `?` inline filtering and `q`/Ctrl+C to quit. Clicking a row copies
//! the config path, matching the "entries behave like links" requirement.

use crate::ui::{clipboard, term::TuiGuard};
use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::io::Stdout;
use vmate_core::db::models::StoredConfig;

type Term = Terminal<CrosstermBackend<Stdout>>;

/// Run the recent-configs TUI. Restores the terminal on every exit path.
pub fn run(entries: Vec<StoredConfig>) -> Result<()> {
    enable_raw_mode()?;
    execute!(
        std::io::stdout(),
        EnterAlternateScreen,
        event::EnableMouseCapture
    )?;
    let _guard = TuiGuard;

    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = RecentApp::new(entries);
    app.event_loop(&mut terminal)
}

struct RecentApp {
    entries: Vec<StoredConfig>,
    visible: Vec<usize>,
    list_state: ListState,
    list_area: Rect,
    filter: String,
    filter_input: bool,
    status: String,
}

impl RecentApp {
    fn new(entries: Vec<StoredConfig>) -> Self {
        let mut app = Self {
            entries,
            visible: Vec::new(),
            list_state: ListState::default(),
            list_area: Rect::default(),
            filter: String::new(),
            filter_input: false,
            status: String::new(),
        };
        app.rebuild_visible();
        app
    }

    fn rebuild_visible(&mut self) {
        if self.filter.is_empty() {
            self.visible = (0..self.entries.len()).collect();
        } else {
            let needle = self.filter.to_lowercase();
            self.visible = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.path.to_lowercase().contains(&needle)
                        || e.country.to_lowercase().contains(&needle)
                })
                .map(|(i, _)| i)
                .collect();
        }
        self.list_state.select(if self.visible.is_empty() {
            None
        } else {
            Some(0)
        });
    }

    fn event_loop(&mut self, terminal: &mut Term) -> Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            let event = event::read()?;
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press && self.handle_key(key)? => {
                    break;
                }
                Event::Mouse(mouse) => self.handle_mouse(mouse),
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<bool> {
        // Ctrl+C always quits.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }

        if self.filter_input {
            match key.code {
                KeyCode::Char(c) => self.filter.push(c),
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Enter => {
                    self.filter_input = false;
                }
                KeyCode::Esc => {
                    self.filter.clear();
                    self.filter_input = false;
                }
                _ => {}
            }
            self.rebuild_visible();
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Ok(true),
            KeyCode::Char('c') => {
                self.copy_selected()?;
                Ok(false)
            }
            KeyCode::Char('/') => {
                self.filter_input = true;
                Ok(false)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(-1);
                Ok(false)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection(1);
                Ok(false)
            }
            KeyCode::Enter => {
                self.copy_selected()?;
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let row = mouse.row;
                if row >= self.list_area.y && row < self.list_area.y + self.list_area.height {
                    let rel = (row - self.list_area.y) as usize;
                    let idx = self.list_state.offset() + rel;
                    if idx < self.visible.len() {
                        self.list_state.select(Some(idx));
                        let _ = self.copy_selected();
                    }
                }
            }
            MouseEventKind::ScrollUp => self.move_selection(-1),
            MouseEventKind::ScrollDown => self.move_selection(1),
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.visible.len();
        if len == 0 {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, len as isize - 1) as usize;
        self.list_state.select(Some(next));
    }

    fn copy_selected(&mut self) -> Result<()> {
        let Some(pos) = self.list_state.selected() else {
            return Ok(());
        };
        let Some(entry) = self.visible.get(pos).and_then(|i| self.entries.get(*i)) else {
            return Ok(());
        };
        match clipboard::copy_to_clipboard(&entry.path) {
            Ok(method) => self.status = format!("Copied: {} ({method})", entry.path),
            Err(err) => self.status = format!("copy failed: {err}"),
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
        self.list_area = chunks[1];

        let title = if self.filter_input {
            format!(" Recent Configs - filter: {}_", self.filter)
        } else if self.filter.is_empty() {
            format!(" Recent Configs - {} entries", self.visible.len())
        } else {
            format!(" Recent Configs - filter: {}", self.filter)
        };
        frame.render_widget(
            Block::default().borders(Borders::BOTTOM).title(title),
            chunks[0],
        );

        let items: Vec<ListItem> = self
            .visible
            .iter()
            .filter_map(|i| self.entries.get(*i))
            .map(|entry| {
                let last = entry
                    .last_success_at
                    .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "-".to_string());
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:<4}", entry.country),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::raw(entry.path.clone()),
                    Span::raw("  "),
                    Span::styled(last, Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::raw(format!("x{}", entry.success_count)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, chunks[1], &mut self.list_state);

        frame.render_widget(
            Paragraph::new(
                "Enter/click: copy path | c: copy | /: filter | up/down: move | q: quit",
            )
            .style(Style::default().fg(Color::DarkGray)),
            chunks[2],
        );

        let style = if self.status.starts_with("Copied:") {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };
        frame.render_widget(Paragraph::new(self.status.clone()).style(style), chunks[3]);
    }
}
