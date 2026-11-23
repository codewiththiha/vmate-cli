//! Batch export contracts use isolated history, never network operations.

use std::time::{Duration, Instant};
use vmate_engine::{
    Configuration, Engine, Port,
    model::{Command, ExportItem},
};

fn idle(port: &Port) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while (!port.snapshot().ready || port.snapshot().busy || port.snapshot().exporting)
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(port.snapshot().ready && !port.snapshot().busy);
}

#[test]
fn selected_exports_preserve_existing_files_and_reject_traversal() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("history.db");
    let source = directory.path().join("relay.ovpn");
    std::fs::write(&source, "client\nremote example.invalid 1194\n").unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let pool = vmate_core::db::init_pool(&database).await.unwrap();
        let repo = vmate_core::db::ConfigRepo::new(pool);
        repo.record_success(
            &source,
            "hash",
            None,
            &vmate_core::country::CountryCode::new("JP").unwrap(),
            vmate_core::db::CountrySource::FileName,
        )
        .await
        .unwrap();
    });
    drop(runtime);
    let engine = Engine::start(Configuration {
        config_dir: directory.path().join("config"),
        database,
        openvpn: "/bin/true".into(),
    })
    .unwrap();
    let port = engine.port();
    idle(&port);
    let destination = directory.path().join("exports");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("JP.ovpn"), "do not replace").unwrap();
    let item = ExportItem {
        path: source.clone(),
        filename: "JP.ovpn".into(),
    };
    port.send(Command::ExportBatch {
        items: vec![item.clone(), item],
        directory: destination.clone(),
    })
    .unwrap();
    idle(&port);
    assert!(
        port.snapshot().error.is_empty(),
        "{}",
        port.snapshot().error
    );
    assert_eq!(
        std::fs::read_to_string(destination.join("JP.ovpn")).unwrap(),
        "do not replace"
    );
    for name in ["JP_1.ovpn", "JP_2.ovpn"] {
        assert_eq!(
            std::fs::read(destination.join(name)).unwrap(),
            std::fs::read(&source).unwrap()
        );
    }
    port.send(Command::ExportBatch {
        items: vec![ExportItem {
            path: source,
            filename: "../escape.ovpn".into(),
        }],
        directory: destination,
    })
    .unwrap();
    idle(&port);
    assert!(port.snapshot().error.contains("Invalid export filename"));
    assert!(!directory.path().join("escape.ovpn").exists());
    drop(engine);
}

#[test]
fn filename_styles_match_the_dialog_contract() {
    use vmate_engine::export::filename;
    assert_eq!(filename("relay.ovpn", "JP", 0), "relay.ovpn");
    assert_eq!(filename("relay", "JP", 1), "JP.ovpn");
    assert_eq!(filename("relay", "JP", 2), "relay_JP.ovpn");
    assert_eq!(filename("relay", "japan", 1), "japan.ovpn");
    assert_eq!(filename("relay", "japan", 2), "relay_japan.ovpn");
    assert_eq!(filename("../../relay", "JP", 2), ".._.._relay_JP.ovpn");
}
