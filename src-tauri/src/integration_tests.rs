//! Integration tests against the real operating system. They need a desktop
//! session and are ignored by default; run them with
//! `cargo test -- --ignored --test-threads=1`.

#[cfg(target_os = "linux")]
mod linux {
    use std::sync::atomic::AtomicBool;
    use std::sync::{mpsc, Arc};
    use std::time::{Duration, Instant};

    use crate::platform;
    use crate::search::files::index::{IndexConfig, IndexData};
    use crate::search::fuzzy::Query;
    use crate::shortcuts::accelerator::Accelerator;
    use crate::shortcuts::backend::{RegisterError, ShortcutBackend};

    /// Registers, conflicts and releases shortcuts through Gala (elementary OS).
    #[test]
    #[ignore]
    fn gala_global_shortcut_registration() {
        if !platform::linux::session::is_pantheon() {
            eprintln!("skipped: not an elementary OS session");
            return;
        }
        let backend = crate::shortcuts::gala_backend::GalaBackend::connect(Arc::new(|_, _| {})).expect("Gala available");
        let free: Accelerator = "Ctrl+Alt+Shift+F12".parse().unwrap();
        backend.register(free).expect("an unused combination registers");
        backend.unregister(free).expect("and is released again");

        let taken: Accelerator = "Alt+Space".parse().unwrap();
        match backend.register(taken) {
            Err(RegisterError::Conflict(message)) => assert!(message.contains("window menu"), "{message}"),
            other => {
                let _ = backend.unregister(taken);
                panic!("expected a conflict with the window menu shortcut, got {other:?}");
            }
        }
    }

    /// Launches a real desktop entry through GIO.
    #[test]
    #[ignore]
    fn launches_desktop_entry() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("launched");
        let entry = dir.path().join("lumio-test.desktop");
        std::fs::write(
            &entry,
            format!("[Desktop Entry]\nType=Application\nName=Lumio Test\nExec=touch {}\n", marker.display()),
        )
        .unwrap();
        platform::launch::launch_app(&entry.to_string_lossy()).expect("launch succeeds");
        let start = Instant::now();
        while !marker.exists() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(marker.exists(), "the launched program ran");
        assert!(platform::launch::launch_app("/no/such/app.desktop").is_err());
    }

    /// The live Caps Lock monitor reports the current state immediately.
    #[test]
    #[ignore]
    fn caps_lock_monitor_reports_initial_state() {
        let (tx, rx) = mpsc::channel();
        let method = platform::capslock::start_monitor(
            Arc::new(move |on| {
                let _ = tx.send(on);
            }),
            Arc::new(AtomicBool::new(true)),
        )
        .expect("monitoring starts");
        let state = rx.recv_timeout(Duration::from_secs(2)).expect("initial reading");
        eprintln!("caps lock is {} (via {method})", if state { "on" } else { "off" });
    }

    /// The input method (IBus) accepts Lumio's watch for text cursor positions.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn input_method_watch_connects() {
        let watch = platform::ime::ImeWatch::start(|event| eprintln!("{event:?}")).expect("IBus is running");
        assert!(watch.is_alive());
    }

    /// Creating a file in a watched folder becomes searchable without a full rescan.
    #[test]
    #[ignore]
    fn file_index_picks_up_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let config = IndexConfig { roots: vec![dir.path().to_path_buf()], exclusions: vec![], include_hidden: false, max_entries: 1000, max_depth: 4 };
        let mut data = IndexData::build(&config, &|| false);
        assert!(data.search(&Query::new("quarterly"), true, true, 5).is_empty());
        std::fs::write(dir.path().join("quarterly-report.txt"), b"x").unwrap();
        data.rescan(&dir.path().to_string_lossy(), &config);
        assert_eq!(data.search(&Query::new("quarterly"), true, true, 5).len(), 1);
    }
}
