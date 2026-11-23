//! Existing-profile NetworkManager command with terminal presentation.

use anyhow::Result;
use tokio_util::sync::CancellationToken;

pub async fn run(uuid: &str) -> Result<()> {
    let mut host = crate::ui::connect::host(false, String::new(), false)?;
    let cancel = CancellationToken::new();
    let _signal = vmate_core::system::signal::cancel_on_shutdown(cancel.clone());
    let result = vmate_core::managed::run(uuid, host.as_mut(), cancel).await;
    host.finish().await?;
    result
}
