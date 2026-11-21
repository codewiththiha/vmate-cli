use super::*;
use crate::connect::host::UserCommand;
use crate::connect::queue::Candidate;
use crate::connect::state::ConnectionStatus;
use crate::country::CountryCode;
use crate::db::models::CountrySource;
use crate::db::pool::init_pool;
use async_trait::async_trait;
use std::sync::Mutex;

struct FakeHost {
    commands: Mutex<std::vec::IntoIter<UserCommand>>,
    messages: Mutex<Vec<String>>,
    statuses: Mutex<Vec<ConnectionStatus>>,
}

impl FakeHost {
    fn new(commands: Vec<UserCommand>) -> Self {
        Self {
            commands: Mutex::new(commands.into_iter()),
            messages: Mutex::new(Vec::new()),
            statuses: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ConnectHost for FakeHost {
    async fn status(&mut self, status: &ConnectionStatus) -> Result<()> {
        self.statuses.lock().unwrap().push(status.clone());
        Ok(())
    }
    async fn notify(&mut self, message: &str) -> Result<()> {
        self.messages.lock().unwrap().push(message.to_string());
        Ok(())
    }
    async fn log(&mut self, _line: &str) -> Result<()> {
        Ok(())
    }
    async fn copy(&mut self, _text: &str) -> Result<()> {
        Ok(())
    }
    async fn poll_command(&mut self) -> Option<UserCommand> {
        // Yield like the real host's blocking key poll (up to 200ms) so
        // concurrent producers — the openvpn output reader and the connect
        // timer — get scheduled instead of the select loop spinning.
        tokio::time::sleep(Duration::from_millis(10)).await;
        self.commands.lock().unwrap().next()
    }
    async fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Spawning always fails (no OpenVPN available in tests).
struct NeverSpawn;

impl OpenVpnRunner for NeverSpawn {
    fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
        Err(anyhow::anyhow!("no openvpn in tests"))
    }
    fn bin(&self) -> &str {
        "openvpn"
    }
}

/// Spawns `sh -c 'echo "Initialization Sequence Completed"'`: the handshake
/// succeeds, then the process exits immediately, so the connected session
/// crashes right after connecting (a connect-then-crash).
struct ConnectThenExitRunner {
    registry: Arc<ProcessRegistry>,
}

impl OpenVpnRunner for ConnectThenExitRunner {
    fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
        crate::ovpn::process::spawn_openvpn(
            "sh",
            &[
                "-c".into(),
                "echo 'Initialization Sequence Completed'".into(),
            ],
            &self.registry,
        )
    }
    fn bin(&self) -> &str {
        "sh"
    }
}

/// Spawns a real `sh -c 'echo AUTH_FAILED'`, so the handshake monitor
/// immediately sees an error line and the connection fails.
struct FailHandshakeRunner {
    registry: Arc<ProcessRegistry>,
}

impl OpenVpnRunner for FailHandshakeRunner {
    fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
        crate::ovpn::process::spawn_openvpn(
            "sh",
            &["-c".into(), "echo AUTH_FAILED".into()],
            &self.registry,
        )
    }
    fn bin(&self) -> &str {
        "sh"
    }
}

/// Spawns `sh -c 'sleep 30'`: no completion or error lines, so the
/// handshake hangs until a key is pressed or the timeout fires.
struct SlowRunner {
    registry: Arc<ProcessRegistry>,
}

impl OpenVpnRunner for SlowRunner {
    fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
        crate::ovpn::process::spawn_openvpn("sh", &["-c".into(), "sleep 30".into()], &self.registry)
    }
    fn bin(&self) -> &str {
        "sh"
    }
}

/// Spawns `sh -c 'echo "Initialization Sequence Completed"; sleep 0.2'`:
/// the handshake succeeds, the session stays up past the stability grace,
/// then the process exits — a long-lived session's crash, not a
/// connect-then-crash.
struct ConnectThenStableThenExitRunner {
    registry: Arc<ProcessRegistry>,
}

impl OpenVpnRunner for ConnectThenStableThenExitRunner {
    fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
        crate::ovpn::process::spawn_openvpn(
            "sh",
            &[
                "-c".into(),
                "echo 'Initialization Sequence Completed'; sleep 0.2".into(),
            ],
            &self.registry,
        )
    }
    fn bin(&self) -> &str {
        "sh"
    }
}

fn test_service(repo: Arc<ConfigRepo>) -> ConnectService {
    let killer: Arc<dyn ProcessKiller> = Arc::new(crate::system::killer::RealProcessKiller {
        killall_enabled: false,
    });
    ConnectService {
        runner: Arc::new(NeverSpawn),
        killer,
        registry: Arc::new(ProcessRegistry::new()),
        repo,
        options: ConnectOptions {
            connect_timeout: Duration::from_secs(1),
            connect_stability_grace: Duration::from_secs(1),
            retry_count: 2,
            killall_enabled: false,
        },
    }
}

struct FailTwiceThenHang {
    spawns: std::sync::atomic::AtomicUsize,
    registry: Arc<ProcessRegistry>,
}

impl OpenVpnRunner for FailTwiceThenHang {
    fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
        use std::sync::atomic::Ordering;
        let n = self.spawns.fetch_add(1, Ordering::SeqCst);
        if n < 2 {
            crate::ovpn::process::spawn_openvpn(
                "sh",
                &["-c".into(), "echo AUTH_FAILED".into()],
                &self.registry,
            )
        } else {
            crate::ovpn::process::spawn_openvpn(
                "sh",
                &["-c".into(), "sleep 30".into()],
                &self.registry,
            )
        }
    }
    fn bin(&self) -> &str {
        "sh"
    }
}

/// A host that returns `q` once it has observed the N-th
/// "Retrying ... (failure N/M)" status, letting the in-flight retry be
/// interrupted deterministically.
struct QuitAfterRetries {
    failures_seen: usize,
    quit_after: usize,
}

#[async_trait]
impl ConnectHost for QuitAfterRetries {
    async fn status(&mut self, s: &ConnectionStatus) -> Result<()> {
        if let Some(rest) = s.message.split("(failure ").nth(1) {
            if let Some(n) = rest.split('/').next().and_then(|n| n.parse::<usize>().ok()) {
                self.failures_seen = self.failures_seen.max(n);
            }
        }
        Ok(())
    }
    async fn notify(&mut self, _message: &str) -> Result<()> {
        Ok(())
    }
    async fn log(&mut self, _line: &str) -> Result<()> {
        Ok(())
    }
    async fn copy(&mut self, _text: &str) -> Result<()> {
        Ok(())
    }
    async fn poll_command(&mut self) -> Option<UserCommand> {
        tokio::time::sleep(Duration::from_millis(10)).await;
        if self.failures_seen >= self.quit_after {
            Some(UserCommand::Quit)
        } else {
            None
        }
    }
    async fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}

/// A configurable retry budget: with `retry_count: 3`, a repeatedly-failing
/// config survives two failures and is only dropped on the third.
struct QuitAfterConnect {
    connects: usize,
    quit_after: usize,
}

#[async_trait]
impl ConnectHost for QuitAfterConnect {
    async fn status(&mut self, s: &ConnectionStatus) -> Result<()> {
        if s.connected {
            self.connects += 1;
        }
        Ok(())
    }
    async fn notify(&mut self, _message: &str) -> Result<()> {
        Ok(())
    }
    async fn log(&mut self, _line: &str) -> Result<()> {
        Ok(())
    }
    async fn copy(&mut self, _text: &str) -> Result<()> {
        Ok(())
    }
    async fn poll_command(&mut self) -> Option<UserCommand> {
        // Yield like the real host's blocking key poll so the crash timer
        // and output reader get scheduled instead of the select loop
        // spinning.
        tokio::time::sleep(Duration::from_millis(10)).await;
        if self.connects >= self.quit_after {
            Some(UserCommand::Quit)
        } else {
            None
        }
    }
    async fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}

/// A config that connects and stays up past the stability grace must NOT be
/// dropped for two long-session crashes: each crash resets the failure
/// budget, so the config keeps being retried (Go parity). Resetting on
/// every handshake would loop forever; never resetting would delete a
/// working config after two network blips.
mod budget;
mod handshake;
mod sessions;
