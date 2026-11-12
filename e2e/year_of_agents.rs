//! End to end against a real haunt core. Copy into haunt's `cli/tests/` and run
//! `cargo test -p haunt-cli --features plugins --test year_of_agents` (see README).
#![cfg(feature = "plugins")]

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use haunt_core::plugin_settings::PluginConfig;
use haunt_core::plugin_panel::{PanelNodeKind, PanelPlacement};
use haunt_core::plugins::HostRequest;
use haunt_core::plugin_store::{Store, attach};
use haunt_core::plugins::PluginHost;
use haunt_core::{Core, SurfaceEnv};

const ID: &str = "dev.haunt.year-of-agents";

fn repo() -> PathBuf {
    std::env::var_os("HAUNT_PLUGINS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../haunt-plugins"))
        .join("haunt-year-of-agents")
}

fn send(sock: &mut UnixStream, env: &SurfaceEnv, nonce: bool, cwd: &str, event: &str) {
    let nonce = if nonce { format!(r#","nonce":"{}""#, env.nonce) } else { String::new() };
    let line = format!(r#"{{"v":1,"agent":"claude","surface_id":"{}"{nonce},"session_id":"s1","cwd":"{cwd}","event":"{event}"}}"#, env.surface_id);
    sock.write_all(format!("{line}\n").as_bytes()).unwrap();
}

#[test]
fn finished_turns_fill_todays_square_in_the_pane() {
    let dir = repo();
    if !dir.join("haunt-plugin.toml").is_file() {
        eprintln!("skipped: haunt-year-of-agents not found; set HAUNT_PLUGINS_DIR");
        return;
    }
    let home = PathBuf::from(format!("/tmp/haunt-fp-year-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "--release", "--target", "wasm32-wasip2"])
        .current_dir(&dir)
        .env_remove("CARGO_TARGET_DIR")
        .status()
        .unwrap();
    assert!(status.success());
    let o = Command::new(env!("CARGO_BIN_EXE_haunt")).args(["plugin", "install", dir.to_str().unwrap(), "--yes"]).env("HAUNT_HOME", &home).output().unwrap();
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));

    let core = Core::new(&home);
    core.start().unwrap();
    let env = core.register_surface("/tmp").unwrap();
    let set = Store::open(&home).unwrap().load_all(&PluginHost::new().unwrap(), Some(&core.handle())).unwrap();
    assert!(set.errors().is_empty(), "{:?}", set.errors());
    let set = Arc::new(Mutex::new(set));
    attach(&core, &set);
    let commands: Vec<_> = set.lock().unwrap().palette_items(None, "").into_iter().map(|i| (i.plugin_id, i.title)).collect();
    assert_eq!(commands, [(ID.to_owned(), "Year of agents".to_owned())]);

    let mut sock = UnixStream::connect(&env.socket_path).unwrap();
    for _ in 0..3 {
        send(&mut sock, &env, true, "/tmp/web", "stop");
    }
    send(&mut sock, &env, false, "/tmp/web", "stop");
    // The events reach the plugin on their own thread; a second check after the forged one lands.
    std::thread::sleep(Duration::from_millis(300));

    set.lock().unwrap().invoke(ID, "show", true).unwrap();
    let panel = set.lock().unwrap().take_requests().into_iter().find_map(|r| match r {
        HostRequest::ShowPanel { plugin_id, panel, .. } if plugin_id == ID => Some(panel),
        _ => None,
    });
    let panel = panel.expect("the pane is shown");
    assert_eq!((panel.id.as_str(), panel.placement, panel.title.as_str()), ("year", PanelPlacement::Right, "Last 26 weeks"));
    let PanelNodeKind::Grid { rows, columns, cells, marked, .. } = &panel.nodes[0].kind else { panic!("{:?}", panel.nodes[0]) };
    assert_eq!((*rows, *columns, marked.len()), (7, 26, 1));
    assert_eq!(cells[marked[0] as usize], 0.5, "3 verified turns today is the second step; the forged one isn't counted");
    let texts: Vec<_> = panel.nodes.iter().filter_map(|n| if let PanelNodeKind::Text { text, .. } = &n.kind { Some(text.as_str()) } else { None }).collect();
    assert_eq!(texts, ["3", "turns", "streak", "1 day", "best", texts[5]]);

    set.lock().unwrap().set_config(&PluginConfig::parse(&format!("[plugins.\"{ID}\"]\nweeks = 4\nstats = false\n")));
    let again = set.lock().unwrap().take_requests().into_iter().find_map(|r| match r {
        HostRequest::ShowPanel { panel, .. } => Some(panel),
        _ => None,
    });
    let again = again.expect("an open pane follows its settings");
    assert_eq!(again.nodes.len(), 1);
    assert!(matches!(again.nodes[0].kind, PanelNodeKind::Grid { columns: 4, .. }));

    set.lock().unwrap().panel_action(ID, "year", again.generation, "").unwrap();
    send(&mut sock, &env, true, "/tmp/web", "stop");
    std::thread::sleep(Duration::from_millis(300));
    assert!(set.lock().unwrap().take_requests().iter().all(|r| !matches!(r, HostRequest::ShowPanel { .. })), "a closed pane stays closed");

    drop(core);
    let _ = std::fs::remove_dir_all(&home);
}
