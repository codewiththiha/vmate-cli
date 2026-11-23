use super::*;

#[derive(Default)]
struct Fake {
    calls: Vec<String>,
    established: bool,
    cancel_in_scan: bool,
    fatal_attempt: bool,
    cancel: CancellationToken,
    stop_on_attempt: bool,
}

#[async_trait]
impl Backend for Fake {
    async fn scan(&mut self) -> Result<Vec<Relay>> {
        self.calls.push("scan".into());
        if self.cancel_in_scan {
            self.cancel.cancel();
        }
        Ok(vec![relay("fresh")])
    }
    async fn attempt(&mut self, relay: &Relay) -> Result<bool> {
        self.calls.push(relay.name.clone());
        if self.fatal_attempt {
            bail!("Authorization or teardown is uncertain");
        }
        if self.stop_on_attempt {
            self.cancel.cancel();
        }
        Ok(self.established)
    }
    async fn discard(&mut self, _: &Relay) -> Result<()> {
        self.calls.push("discard".into());
        Ok(())
    }
    async fn pause(&mut self, _: u32) {
        self.calls.push("pause".into());
    }
}

fn relay(name: &str) -> Relay {
    Relay {
        name: name.into(),
        ..Relay::default()
    }
}

#[tokio::test]
async fn recent_is_tried_before_scanning_and_exhaustion_is_bounded() {
    let mut fake = Fake::default();
    assert!(
        drive(
            &mut fake,
            vec![relay("newer"), relay("older")],
            true,
            2,
            CancellationToken::new()
        )
        .await
        .is_err()
    );
    assert_eq!(
        fake.calls,
        [
            "newer", "discard", "older", "discard", "scan", "fresh", "discard", "pause", "scan",
            "fresh", "discard"
        ]
    );
}

#[tokio::test]
async fn empty_history_scans_then_connects_without_manual_action() {
    let mut fake = Fake {
        established: true,
        ..Fake::default()
    };
    assert!(
        drive(&mut fake, vec![], false, 3, CancellationToken::new())
            .await
            .is_err()
    );
    assert_eq!(fake.calls, ["scan", "fresh", "discard"]);
}

#[tokio::test]
async fn disconnect_does_not_remove_a_healthy_recent_or_start_scan() {
    let cancel = CancellationToken::new();
    let mut fake = Fake {
        cancel: cancel.clone(),
        stop_on_attempt: true,
        ..Fake::default()
    };
    drive(&mut fake, vec![relay("recent")], true, 3, cancel)
        .await
        .unwrap();
    assert_eq!(fake.calls, ["recent"]);
}

#[tokio::test]
async fn lost_connection_without_reconnect_does_not_scan() {
    let mut fake = Fake {
        established: true,
        ..Fake::default()
    };
    assert!(
        drive(
            &mut fake,
            vec![relay("recent")],
            false,
            3,
            CancellationToken::new()
        )
        .await
        .is_err()
    );
    assert_eq!(fake.calls, ["recent", "discard"]);
}

#[tokio::test]
async fn reconnect_moves_to_next_recent_before_rescanning() {
    let mut fake = Fake {
        established: true,
        ..Fake::default()
    };
    assert!(
        drive(
            &mut fake,
            vec![relay("first"), relay("next")],
            true,
            1,
            CancellationToken::new()
        )
        .await
        .is_err()
    );
    assert_eq!(
        fake.calls,
        [
            "first", "discard", "next", "discard", "scan", "fresh", "discard"
        ]
    );
}

#[tokio::test]
async fn cancelled_scan_never_starts_a_connection() {
    let cancel = CancellationToken::new();
    let mut fake = Fake {
        cancel: cancel.clone(),
        cancel_in_scan: true,
        ..Fake::default()
    };
    drive(&mut fake, vec![], true, 2, cancel).await.unwrap();
    assert_eq!(fake.calls, ["scan"]);
}

#[tokio::test]
async fn fatal_service_errors_do_not_discard_history_or_retry() {
    let mut fake = Fake {
        fatal_attempt: true,
        ..Fake::default()
    };
    assert!(
        drive(
            &mut fake,
            vec![relay("recent")],
            true,
            3,
            CancellationToken::new()
        )
        .await
        .is_err()
    );
    assert_eq!(fake.calls, ["recent"]);
}
