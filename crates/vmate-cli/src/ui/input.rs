//! Bounded terminal input owned independently of VPN output processing.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use vmate_core::connect::UserCommand;

/// A single terminal reader with bounded events and priority quit delivery.
pub(super) struct InputReader {
    events: mpsc::Receiver<UserCommand>,
    quit: CancellationToken,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl InputReader {
    pub(super) fn new(enabled: bool) -> Result<Self> {
        let (sender, events) = mpsc::channel(32);
        let quit = CancellationToken::new();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker_quit = quit.clone();
        let thread = if enabled {
            Some(
                std::thread::Builder::new()
                    .name("vmate-input".into())
                    .spawn(move || {
                        while !worker_stop.load(Ordering::Acquire) {
                            match event::poll(Duration::from_millis(20)) {
                                Ok(false) => continue,
                                Err(_) => break,
                                Ok(true) => {}
                            }
                            let Ok(Event::Key(key)) = event::read() else {
                                continue;
                            };
                            if let Some(command) = command(key) {
                                if command == UserCommand::Quit {
                                    worker_quit.cancel();
                                } else {
                                    let _ = sender.try_send(command);
                                }
                            }
                        }
                    })?,
            )
        } else {
            None
        };
        Ok(Self {
            events,
            quit,
            stop,
            thread,
        })
    }

    pub(super) async fn next(&mut self) -> Option<UserCommand> {
        tokio::select! {
            biased;
            _ = self.quit.cancelled() => Some(UserCommand::Quit),
            Some(command) = self.events.recv() => Some(command),
        }
    }
}

impl Drop for InputReader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn command(key: KeyEvent) -> Option<UserCommand> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
    {
        return Some(UserCommand::Quit);
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(UserCommand::Quit),
        KeyCode::Char('n') => Some(UserCommand::Next),
        KeyCode::Char('r') => Some(UserCommand::Reconnect),
        KeyCode::Char('c') => Some(UserCommand::CopyPath),
        KeyCode::Char('v') => Some(UserCommand::ToggleVerbose),
        KeyCode::Char('?') => Some(UserCommand::Help),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_quit_forms_have_the_same_contract() {
        assert_eq!(
            command(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(UserCommand::Quit)
        );
        assert_eq!(
            command(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(UserCommand::Quit)
        );
    }

    #[tokio::test]
    async fn quit_does_not_wait_behind_a_full_queue() {
        let mut input = InputReader::new(false).unwrap();
        input.quit.cancel();
        assert_eq!(input.next().await, Some(UserCommand::Quit));
    }
}
