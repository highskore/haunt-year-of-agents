//! Finished turns per local day, and the 26-week view, free of host calls so it tests natively.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const DAY_S: i64 = 86_400;
/// Days are kept for a little over a year; the view shows at most 53 weeks.
pub const KEEP_DAYS: i64 = 400;

/// Turn counts keyed by day number (days since 1970-01-01 in local time).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub days: BTreeMap<i64, u32>,
}

pub fn day_of(at_ms: u64, offset_min: i32) -> i64 {
    (at_ms as i64 / 1000 + i64::from(offset_min) * 60).div_euclid(DAY_S)
}

/// 0 is Sunday.
pub fn weekday(day: i64) -> i64 {
    (day + 4).rem_euclid(7)
}

impl History {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn record(&mut self, day: i64) {
        let n = self.days.entry(day).or_default();
        *n = n.saturating_add(1);
        self.days.retain(|d, _| *d > day - KEEP_DAYS);
    }

    pub fn count(&self, day: i64) -> u32 {
        self.days.get(&day).copied().unwrap_or(0)
    }

    /// Consecutive days with a finished turn, ending today, or yesterday while today is still empty.
    pub fn streak(&self, today: i64) -> u32 {
        let mut d = if self.count(today) > 0 { today } else { today - 1 };
        let mut n = 0;
        while self.count(d) > 0 {
            n += 1;
            d -= 1;
        }
        n
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// Columns are weeks, oldest first, each Sunday to Saturday; `None` is a day after today.
    pub weeks: Vec<[Option<Cell>; 7]>,
    pub total: u64,
    pub streak: u32,
    /// The busiest day in the view and its count; `None` with no turns.
    pub best: Option<(i64, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub day: i64,
    pub count: u32,
    /// 0 (none), then 1-2, 3-5, 6-10 and 11+ turns.
    pub level: u8,
    pub today: bool,
}

pub fn level(count: u32) -> u8 {
    match count {
        0 => 0,
        1..=2 => 1,
        3..=5 => 2,
        6..=10 => 3,
        _ => 4,
    }
}

impl History {
    pub fn view(&self, today: i64, weeks: usize) -> View {
        let weeks = weeks.clamp(1, 53);
        let first = today - weekday(today) - 7 * (weeks as i64 - 1);
        let in_view = |d: i64| d >= first && d <= today;
        let max = self.days.iter().filter(|(d, _)| in_view(**d)).map(|(_, n)| *n).max().unwrap_or(0);
        let best = self.days.iter().filter(|(d, n)| in_view(**d) && **n == max && max > 0).map(|(d, n)| (*d, *n)).next_back();
        let total = self.days.iter().filter(|(d, _)| in_view(**d)).map(|(_, n)| u64::from(*n)).sum();
        let weeks = (0..weeks as i64)
            .map(|w| {
                std::array::from_fn(|i| {
                    let day = first + 7 * w + i as i64;
                    (day <= today).then(|| Cell { day, count: self.count(day), level: level(self.count(day)), today: day == today })
                })
            })
            .collect();
        View { weeks, total, streak: self.streak(today), best }
    }
}

/// `(year, month 1-12, day 1-31)` of a day number (Howard Hinnant's civil_from_days).
pub fn civil(day: i64) -> (i64, u32, u32) {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// "Oct 14".
pub fn short_date(day: i64) -> String {
    let (_, m, d) = civil(day);
    format!("{} {d}", MONTHS[m as usize - 1])
}

/// "1,284".
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-11-12 is a Thursday.
    const NOV_12: i64 = 20_769;

    #[test]
    fn calendar_helpers() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(NOV_12), (2026, 11, 12));
        assert_eq!(weekday(NOV_12), 4, "a Thursday");
        assert_eq!(weekday(0), 4, "1970-01-01 was a Thursday");
        assert_eq!(weekday(NOV_12 - 4), 0, "Nov 8 2026 is a Sunday");
        assert_eq!(short_date(NOV_12 - 29), "Oct 14");
        assert_eq!((thousands(7), thousands(1284), thousands(1_000_000)), ("7".into(), "1,284".into(), "1,000,000".into()));
    }

    #[test]
    fn days_follow_the_local_offset() {
        let midnight_utc = NOV_12 as u64 * DAY_S as u64 * 1000;
        assert_eq!(day_of(midnight_utc, 0), NOV_12);
        assert_eq!(day_of(midnight_utc, -60), NOV_12 - 1, "23:00 the evening before in UTC-1");
        assert_eq!(day_of(midnight_utc - 1000, 60), NOV_12, "00:59:59 in UTC+1");
    }

    #[test]
    fn streak_counts_back_from_today_or_yesterday() {
        let mut h = History::default();
        assert_eq!(h.streak(NOV_12), 0);
        for d in [NOV_12 - 1, NOV_12 - 2, NOV_12 - 4] {
            h.record(d);
        }
        assert_eq!(h.streak(NOV_12), 2, "today still empty keeps yesterday's streak");
        h.record(NOV_12);
        assert_eq!(h.streak(NOV_12), 3);
        assert_eq!(h.streak(NOV_12 + 2), 0);
    }

    #[test]
    fn levels_are_fixed_steps() {
        assert_eq!([0, 1, 2, 3, 5, 6, 10, 11, 500].map(level), [0, 1, 1, 2, 2, 3, 3, 4, 4]);
    }

    #[test]
    fn the_view_ends_on_today_and_counts_only_what_it_shows() {
        let mut h = History::default();
        h.days.insert(NOV_12 - 400, 99);
        for _ in 0..3 {
            h.record(NOV_12 - 29);
        }
        h.record(NOV_12);
        let v = h.view(NOV_12, 26);
        assert_eq!(v.weeks.len(), 26);
        let last = v.weeks.last().unwrap();
        assert_eq!(last[4].unwrap().day, NOV_12);
        assert!(last[4].unwrap().today && last[5].is_none() && last[6].is_none(), "no cells after today");
        assert_eq!(v.weeks[0][0].unwrap().day, NOV_12 - 4 - 7 * 25, "starts on a Sunday");
        assert_eq!((v.total, v.best, v.streak), (4, Some((NOV_12 - 29, 3)), 1), "the day 400 back is out of view");
        assert_eq!((last[4].unwrap().level, v.weeks[21][3].unwrap().level), (1, 2));
        assert!(!h.days.contains_key(&(NOV_12 - 400)), "recording drops days past the keep window");
    }

    #[test]
    fn ties_for_best_go_to_the_latest_day() {
        let mut h = History::default();
        h.record(NOV_12 - 5);
        h.record(NOV_12 - 2);
        assert_eq!(h.view(NOV_12, 26).best, Some((NOV_12 - 2, 1)));
        assert_eq!(History::default().view(NOV_12, 26).best, None);
    }

    #[test]
    fn history_round_trips() {
        let mut h = History::default();
        h.record(NOV_12);
        assert_eq!(History::parse(&h.to_json()).unwrap(), h);
        assert!(History::parse("nope").is_err());
    }
}
