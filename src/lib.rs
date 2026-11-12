//! Year of agents: a calendar heatmap of finished agent turns, one square per day, in a pane beside
//! the terminal. Counts live in kv.
// wit-bindgen's flattened `notify` import has 14 params.
#![allow(clippy::too_many_arguments)]

wit_bindgen::generate!({ path: "wit/haunt-plugin.wit", world: "plugin" });

mod year;

use std::sync::Mutex;

use haunt::plugin::log::log;
use haunt::plugin::panel::{self, GridNode, Node, PanelNode, Placement, TextNode, Tone};
use haunt::plugin::types::LogLevel;
use haunt::plugin::{clock, kv, palette};
use serde::Deserialize;
use year::{History, View, day_of, short_date, thousands};

const KEY: &str = "days";
const PANEL: &str = "year";
const CELL: u32 = 9;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub weeks: i64,
    pub stats: bool,
    pub unverified: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { weeks: 26, stats: true, unverified: false }
    }
}

#[derive(Default)]
struct Live {
    settings: Settings,
    open: bool,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Live) -> R) -> R {
    f(LIVE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Live::default))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn today() -> i64 {
    day_of(now_ms(), clock::local_offset_min())
}

fn load() -> History {
    kv::get(KEY)
        .map(|j| {
            History::parse(&j).unwrap_or_else(|e| {
                log(LogLevel::Warn, &format!("year-of-agents: saved counts unreadable ({e}); starting fresh"));
                History::default()
            })
        })
        .unwrap_or_default()
}

fn text(parent: Option<u32>, text: String, tone: Tone) -> PanelNode {
    PanelNode { parent, node: Node::Text(TextNode { text, tone, mono: true, bold: false }) }
}

/// The pane: the grid, Sunday on top, then `1,284 turns · streak 23 days · best Oct 14 · 71`.
pub fn nodes(v: &View, stats: bool) -> Vec<PanelNode> {
    let columns = v.weeks.len() as u32;
    let mut cells = vec![-1.0f32; 7 * columns as usize];
    let mut marked = Vec::new();
    for (c, week) in v.weeks.iter().enumerate() {
        for (r, cell) in week.iter().enumerate() {
            let Some(cell) = cell else { continue };
            let i = r * columns as usize + c;
            cells[i] = f32::from(cell.level) / 4.0;
            if cell.today {
                marked.push(i as u32);
            }
        }
    }
    let mut out = vec![PanelNode { parent: None, node: Node::Grid(GridNode { rows: 7, columns, cell: CELL, cells, marked }) }];
    if stats {
        out.push(PanelNode { parent: None, node: Node::Hstack });
        let row = Some(1);
        let streak = if v.streak == 1 { "1 day".to_owned() } else { format!("{} days", v.streak) };
        let mut pair = |value: String, label: &str, value_first: bool| {
            out.push(PanelNode { parent: row, node: Node::Hstack });
            let at = Some(out.len() as u32 - 1);
            let (a, b) = if value_first { (text(at, value, Tone::Text), text(at, label.into(), Tone::Faint)) } else { (text(at, label.into(), Tone::Faint), text(at, value, Tone::Text)) };
            out.push(a);
            out.push(b);
        };
        pair(thousands(v.total), if v.total == 1 { "turn" } else { "turns" }, true);
        pair(streak, "streak", false);
        let best = v.best.map_or_else(|| "none yet".to_owned(), |(d, n)| format!("{} · {n}", short_date(d)));
        pair(best, "best", false);
    }
    out
}

/// Haunt's own header already names the plugin, so the pane says what it covers.
pub fn title(weeks: usize) -> String {
    if weeks == 1 { "Last week".into() } else { format!("Last {weeks} weeks") }
}

fn show(s: &Settings) {
    let weeks = s.weeks.clamp(1, 53) as usize;
    let v = load().view(today(), weeks);
    if let Err(e) = panel::show(PANEL, Placement::Right, &title(weeks), &nodes(&v, s.stats)) {
        log(LogLevel::Warn, &format!("year-of-agents: pane not shown: {e}"));
    }
}

struct Plugin;

impl Guest for Plugin {
    fn init(json: String) {
        with(|l| l.settings = serde_json::from_str(&json).unwrap_or_default());
        palette::command("show", "Year of agents", None);
    }

    fn on_settings_changed(json: String) {
        palette::command("show", "Year of agents", None);
        with(|l| {
            l.settings = serde_json::from_str(&json).unwrap_or_default();
            if l.open {
                show(&l.settings);
            }
        });
    }

    fn on_event(event: AgentEvent) {
        if event.kind != "stop" {
            return;
        }
        with(|l| {
            if !event.verified && !l.settings.unverified {
                return;
            }
            let mut h = load();
            h.record(today());
            if let Err(e) = kv::set(KEY, &h.to_json()) {
                log(LogLevel::Warn, &format!("year-of-agents: count not saved: {e}"));
            }
            if l.open {
                show(&l.settings);
            }
        });
    }

    fn palette_items(_mode: String, _query: String) -> Vec<PaletteItem> {
        Vec::new()
    }

    fn on_action(action_id: String, payload: String) {
        with(|l| match (action_id.as_str(), payload.as_str()) {
            ("show", _) => {
                l.open = true;
                show(&l.settings);
            }
            ("", p) if p == format!("panel:{PANEL}") => l.open = false,
            _ => {}
        });
    }

    fn row_menu(_surface: String) -> Vec<MenuItem> {
        Vec::new()
    }

    fn on_row_moved(_surface: String, _group: String, _before: Option<String>) {}

    fn on_theme_changed() {}

    fn on_tick(_now_ms: u64) {}

    fn on_exec(_job: u64, _outcome: Result<haunt::plugin::exec::Output, String>) {}

    fn on_away_changed(_away: bool, _away_ms: u64) {}

    fn on_hover_hold(_held: bool, _surface: String) {}

    fn on_fs_tree(_job: u64, _outcome: Result<haunt::plugin::fs_list::TreeResult, String>) {}
}

export!(Plugin);

#[cfg(test)]
mod tests {
    use super::*;

    const NOV_12: i64 = 20_769;

    fn texts(n: &[PanelNode]) -> Vec<String> {
        n.iter().filter_map(|p| if let Node::Text(t) = &p.node { Some(t.text.clone()) } else { None }).collect()
    }

    #[test]
    fn the_pane_reads_like_the_mockup() {
        let mut h = History::default();
        for _ in 0..12 {
            h.record(NOV_12 - 29);
        }
        h.record(NOV_12);
        h.record(NOV_12 - 1);
        let n = nodes(&h.view(NOV_12, 26), true);
        let Node::Grid(g) = &n[0].node else { panic!("a grid first") };
        assert_eq!((g.rows, g.columns, g.cells.len()), (7, 26, 182));
        let at = |r: usize, c: usize| g.cells[r * 26 + c];
        assert_eq!(at(4, 25), 0.25, "today, a Thursday, in the last column");
        assert_eq!(g.marked, [4 * 26 + 25]);
        assert_eq!(at(3, 21), 1.0, "Oct 14, a Wednesday: 12 turns is the top step");
        assert_eq!((at(5, 25), at(6, 25)), (-1.0, -1.0), "no squares after today");
        assert_eq!(at(0, 0), 0.0);
        assert_eq!(texts(&n), ["14", "turns", "streak", "2 days", "best", "Oct 14 · 12"]);
        for p in &n[1..] {
            assert!(p.parent.is_none_or(|i| matches!(n[i as usize].node, Node::Hstack) && (i as usize) < n.len()));
        }
    }

    #[test]
    fn stats_can_be_turned_off_and_start_empty() {
        let v = History::default().view(NOV_12, 26);
        assert_eq!(nodes(&v, false).len(), 1);
        assert_eq!(texts(&nodes(&v, true)), ["0", "turns", "streak", "0 days", "best", "none yet"]);
    }

    #[test]
    fn the_pane_title_names_the_span() {
        assert_eq!((title(26).as_str(), title(1).as_str()), ("Last 26 weeks", "Last week"));
    }

    #[test]
    fn manifest_asks_for_what_it_uses_and_defaults_match() {
        let m = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/haunt-plugin.toml")).unwrap();
        let t: toml::Table = toml::from_str(&m).unwrap();
        let caps = t["capabilities"].as_table().unwrap();
        assert_eq!(caps.keys().collect::<Vec<_>>(), ["events", "panel", "timer"]);
        assert_eq!(caps["events"].as_array().unwrap().len(), 1);
        let defaults: serde_json::Map<String, serde_json::Value> =
            t["settings"].as_table().unwrap().iter().map(|(k, v)| (k.clone(), serde_json::to_value(&v["default"]).unwrap())).collect();
        assert_eq!(serde_json::from_value::<Settings>(serde_json::Value::Object(defaults)).unwrap(), Settings::default());
    }
}
