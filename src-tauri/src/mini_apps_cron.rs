//! Mini-app cron scheduler (plan 1 P4).
//!
//! Mini-apps with `allow_vault_access: true` may declare `cron_jobs` in their
//! manifest. This module scans the registered vaults on a fixed interval,
//! computes the next fire time for every declared job, and executes due jobs
//! by opening a **hidden** mini-app window. The hidden window runs the same
//! shell as a normal mini-app window, so the app's JavaScript executes its
//! task and reads/writes notes through the existing `postMessage` MCP relay
//! — no new IPC surface and no relaxed sandbox.
//!
//! The schedule is evaluated in the machine's local timezone. Fires are
//! de-duplicated per (vault, app, task, schedule) so a single slot never runs
//! twice, and the scheduler tolerates sleep/resume (a missed window fires at
//! most once on the next tick).

use crate::mini_apps::{self, MiniAppCronRegistration};
use chrono::{DateTime, Datelike, Duration, Local, Timelike};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::WebviewUrl;

/// How often the scheduler re-evaluates due jobs.
const TICK_SECONDS: i64 = 30;

/// Upper bound for schedule scanning: "next fire" never looks further ahead
/// than one leap year of minutes.
const MAX_SCAN_MINUTES: i64 = 366 * 24 * 60;

pub const MINI_APP_CRON_FIRE_EVENT: &str = "mini-app-cron-fire";

// ── Cron expression parsing ─────────────────────────────────────────────────

/// One parsed cron field: the set of matching values (already expanded from
/// lists/ranges/steps).
type FieldValues = Vec<u32>;

fn parse_cron_field(field: &str, min: u32, max: u32) -> Result<FieldValues, String> {
    let field = field.trim();
    if field.is_empty() {
        return Err(format!("Empty cron field (expected {min}-{max})"));
    }
    let mut values = Vec::new();
    for part in field.split(',') {
        let (range_part, has_step, step) = match part.split_once('/') {
            Some((range, step)) => (
                range,
                true,
                step.parse::<u32>()
                    .map_err(|_| format!("Invalid cron step '{step}' in field '{field}'"))?,
            ),
            None => (part, false, 1),
        };
        if step == 0 {
            return Err(format!("Cron step must be at least 1 in field '{field}'"));
        }
        let (start, end) = if range_part == "*" {
            (min, max)
        } else if let Some((from, to)) = range_part.split_once('-') {
            let start = parse_field_value(from, min, max, field)?;
            let end = parse_field_value(to, min, max, field)?;
            if start > end {
                return Err(format!("Cron range {start}-{end} is inverted in field '{field}'"));
            }
            (start, end)
        } else {
            // A single value is a point in time; with an explicit step
            // ("5/15") it becomes the start of a range up to the field max.
            let value = parse_field_value(range_part, min, max, field)?;
            if has_step { (value, max) } else { (value, value) }
        };
        let mut current = start;
        while current <= end {
            if !values.contains(&current) {
                values.push(current);
            }
            current += step;
        }
    }
    values.sort_unstable();
    if values.is_empty() {
        return Err(format!("Cron field '{field}' matches no values"));
    }
    Ok(values)
}

fn parse_field_value(raw: &str, min: u32, max: u32, field: &str) -> Result<u32, String> {
    raw.trim()
        .parse::<u32>()
        .ok()
        .filter(|value| (min..=max).contains(value))
        .ok_or_else(|| format!("Cron value '{raw}' out of range {min}-{max} in field '{field}'"))
}

/// A parsed five-field cron expression.
#[derive(Debug, Clone, PartialEq)]
pub struct CronSchedule {
    minutes: FieldValues,
    hours: FieldValues,
    days_of_month: FieldValues,
    months: FieldValues,
    days_of_week: FieldValues,
    /// True when the day-of-month field is a restricted (non-`*`) expression.
    dom_restricted: bool,
    /// True when the day-of-week field is a restricted (non-`*`) expression.
    dow_restricted: bool,
}

impl CronSchedule {
    /// Parses a five-field cron expression (`m h dom mon dow`), supporting
    /// `*`, lists, ranges, and `/` steps. Day-of-week accepts 0-7 with both
    /// 0 and 7 meaning Sunday.
    pub fn parse(expression: &str) -> Result<CronSchedule, String> {
        let fields: Vec<&str> = expression.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(format!(
                "Cron expression '{expression}' must have exactly 5 fields (m h dom mon dow)"
            ));
        }
        let days_of_month = parse_cron_field(fields[2], 1, 31)?;
        let days_of_week = parse_cron_field(fields[4], 0, 7)?
            .into_iter()
            // Normalize 7 (Sunday) to 0 so matching uses the 0-6 convention.
            .map(|day| if day == 7 { 0 } else { day })
            .collect::<Vec<u32>>();
        Ok(CronSchedule {
            minutes: parse_cron_field(fields[0], 0, 59)?,
            hours: parse_cron_field(fields[1], 0, 23)?,
            dom_restricted: fields[2].trim() != "*",
            days_of_month,
            months: parse_cron_field(fields[3], 1, 12)?,
            dow_restricted: fields[4].trim() != "*",
            days_of_week,
        })
    }

    /// Standard cron day matching: when both day-of-month and day-of-week
    /// are restricted the schedule fires on *either* match; when only one is
    /// restricted it must match.
    fn day_matches(&self, time: &DateTime<Local>) -> bool {
        let dom = time.day();
        let dow = time.weekday().num_days_from_sunday();
        let dom_match = self.days_of_month.contains(&dom);
        let dow_match = self.days_of_week.contains(&dow);
        match (self.dom_restricted, self.dow_restricted) {
            (true, true) => dom_match || dow_match,
            (true, false) => dom_match,
            (false, true) => dow_match,
            (false, false) => true,
        }
    }

    /// True when `time` (truncated to the minute) is a fire time.
    pub fn matches(&self, time: &DateTime<Local>) -> bool {
        self.minutes.contains(&time.minute())
            && self.hours.contains(&time.hour())
            && self.months.contains(&time.month())
            && self.day_matches(time)
    }

    /// The first fire time strictly after `after`, scanning up to one leap
    /// year. Returns `None` for schedules that never fire in that window.
    pub fn next_fire_after(&self, after: DateTime<Local>) -> Option<DateTime<Local>> {
        let mut candidate = after
            .with_second(0)
            .and_then(|time| time.with_nanosecond(0))
            .unwrap_or(after)
            + Duration::minutes(1);
        for _ in 0..MAX_SCAN_MINUTES {
            if self.matches(&candidate) {
                return Some(candidate);
            }
            candidate += Duration::minutes(1);
        }
        None
    }
}

// ── Scheduler state ─────────────────────────────────────────────────────────

/// Next scheduled fire (unix seconds) per job key. Jobs are keyed by
/// `vault|app|task|schedule` so the same task declared for two vaults fires
/// independently.
#[derive(Default)]
pub struct CronSchedulerState(Mutex<HashMap<String, i64>>);

fn job_key(job: &MiniAppCronRegistration) -> String {
    format!("{}|{}|{}|{}", job.vault_path, job.app_id, job.task, job.schedule)
}

/// Fired-job record emitted with [`MINI_APP_CRON_FIRE_EVENT`] and used to
/// open the hidden run window.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct MiniAppCronFire {
    pub app_id: String,
    pub app_name: String,
    pub vault_path: String,
    pub task: String,
    pub target_note: Option<String>,
    pub fired_at: i64,
}

fn try_fire_due_jobs(
    app_handle: &tauri::AppHandle,
    state: &CronSchedulerState,
    now: DateTime<Local>,
) {
    let Ok(jobs) = mini_apps::list_mini_app_cron_jobs(app_handle.clone()) else {
        return;
    };
    let Ok(mut next_fires) = state.0.lock() else {
        return;
    };

    for job in jobs {
        let key = job_key(&job);
        let schedule = match CronSchedule::parse(&job.schedule) {
            Ok(schedule) => schedule,
            Err(error) => {
                log::warn!("Mini-app '{}' has an invalid cron schedule: {error}", job.app_id);
                next_fires.remove(&key);
                continue;
            }
        };
        let due_at = match next_fires.get(&key).copied() {
            Some(unix) => chrono::DateTime::from_timestamp(unix, 0)
                .map(|time| time.with_timezone(&Local))
                .unwrap_or_else(|| schedule.next_fire_after(now).unwrap_or(now)),
            None => match schedule.next_fire_after(now) {
                Some(next) => next,
                None => continue,
            },
        };
        if now < due_at {
            continue;
        }
        // Slot is due: fire it, then schedule the following occurrence.
        if let Some(next) = schedule.next_fire_after(now) {
            next_fires.insert(key.clone(), next.timestamp());
        } else {
            next_fires.remove(&key);
        }
        fire_job(app_handle, &job, now);
    }

    // Drop bookkeeping for jobs that no longer exist.
    let live_keys: std::collections::HashSet<String> =
        mini_apps::list_mini_app_cron_jobs(app_handle.clone())
            .unwrap_or_default()
            .iter()
            .map(job_key)
            .collect();
    next_fires.retain(|key, _| live_keys.contains(key));
}

fn fire_job(app_handle: &tauri::AppHandle, job: &MiniAppCronRegistration, now: DateTime<Local>) {
    let fire = MiniAppCronFire {
        app_id: job.app_id.clone(),
        app_name: job.app_name.clone(),
        vault_path: job.vault_path.clone(),
        task: job.task.clone(),
        target_note: job.target_note.clone(),
        fired_at: now.timestamp(),
    };
    log::info!(
        "Mini-app cron: firing task '{}' for app '{}'",
        fire.task,
        fire.app_id
    );
    use tauri::Emitter;
    if let Err(error) = app_handle.emit(MINI_APP_CRON_FIRE_EVENT, &fire) {
        log::warn!("Failed to emit mini-app cron event: {error}");
    }
    if let Err(error) = spawn_hidden_run_window(app_handle, &fire) {
        log::warn!("Failed to open mini-app cron run window: {error}");
    }
}

/// Opens the invisible webview that executes one scheduled task. The shell
/// (`MiniAppWindowApp`) passes `cron_task`/`cron_target` into the app context
/// and closes the window once the app posts `mini-app-cron-done`. Its
/// `CRON_RUN_WATCHDOG_MS` (10 minutes) is the force-close backstop so a stuck
/// app cannot leak a hidden window forever.
fn spawn_hidden_run_window(
    app_handle: &tauri::AppHandle,
    fire: &MiniAppCronFire,
) -> Result<(), String> {
    let label = format!(
        "miniapp-cron-{}-{}",
        fire.app_id,
        Local::now().timestamp_millis()
    );
    let mut params = vec![
        "window=mini-app".to_string(),
        format!("appId={}", mini_apps::url_encode(&fire.app_id)),
        format!("vault={}", mini_apps::url_encode(&fire.vault_path)),
        format!("cronTask={}", mini_apps::url_encode(&fire.task)),
    ];
    if let Some(target) = fire.target_note.as_deref().filter(|note| !note.trim().is_empty()) {
        params.push(format!("cronTarget={}", mini_apps::url_encode(target)));
    }
    tauri::WebviewWindowBuilder::new(
        app_handle,
        &label,
        WebviewUrl::App(format!("/?{}", params.join("&")).into()),
    )
    .title(format!("{} · {}", fire.app_name, fire.task))
    .inner_size(480.0, 360.0)
    .resizable(false)
    .visible(false)
    .build()
    .map_err(|error| format!("Failed to open mini-app cron run window: {error}"))?;
    Ok(())
}

/// Long-running scheduler loop. Spawned once at app startup; exits when the
/// app handle is dropped (Tauri tears down the runtime).
pub fn spawn_scheduler(app_handle: tauri::AppHandle) {
    let state = CronSchedulerState::default();
    let spawned = std::thread::Builder::new().name("nabu-mini-app-cron".into()).spawn(move || {
        log::info!("Mini-app cron scheduler started");
        loop {
            let now = Local::now();
            try_fire_due_jobs(&app_handle, &state, now);
            std::thread::sleep(std::time::Duration::from_secs(TICK_SECONDS as u64));
        }
    });
    if spawned.is_err() {
        log::warn!("Failed to start mini-app cron scheduler thread");
    }
}

/// Manual trigger for one scheduled job (the "Run now" action in the
/// Mini-Apps settings). Validates the job still exists in a manifest before
/// firing so the UI cannot invoke arbitrary app ids.
#[tauri::command]
pub fn run_mini_app_cron_job(
    app_handle: tauri::AppHandle,
    vault_path: String,
    app_id: String,
    task: String,
) -> Result<(), String> {
    let jobs = mini_apps::list_mini_app_cron_jobs(app_handle.clone())?;
    let job = jobs
        .into_iter()
        .find(|job| job.vault_path == vault_path && job.app_id == app_id && job.task == task)
        .ok_or_else(|| format!("Mini-app '{app_id}' declares no cron job '{task}'"))?;
    fire_job(&app_handle, &job, Local::now());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn local_time(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
    ) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn parses_five_field_expressions() {
        let schedule = CronSchedule::parse("0 8 * * 1-5").unwrap();
        assert_eq!(schedule.minutes, vec![0]);
        assert_eq!(schedule.hours, vec![8]);
        assert!(!schedule.dom_restricted);
        assert!(schedule.dow_restricted);
    }

    #[test]
    fn rejects_malformed_expressions() {
        assert!(CronSchedule::parse("0 8 * *").is_err());
        assert!(CronSchedule::parse("0 8 * * * *").is_err());
        assert!(CronSchedule::parse("61 8 * * *").is_err());
        assert!(CronSchedule::parse("0 25 * * *").is_err());
        assert!(CronSchedule::parse("*/0 * * * *").is_err());
        assert!(CronSchedule::parse("5-1 * * * *").is_err());
    }

    #[test]
    fn expands_lists_ranges_and_steps() {
        let schedule = CronSchedule::parse("0,30 6-8/2 * 1,7 *").unwrap();
        assert_eq!(schedule.minutes, vec![0, 30]);
        assert_eq!(schedule.hours, vec![6, 8]);
        assert_eq!(schedule.months, vec![1, 7]);
    }

    #[test]
    fn normalizes_dow_seven_to_sunday() {
        let schedule = CronSchedule::parse("0 12 * * 7").unwrap();
        // Sunday 2026-09-06 noon.
        assert!(schedule.matches(&local_time(2026, 9, 6, 12, 0)));
        assert!(!schedule.matches(&local_time(2026, 9, 7, 12, 0)));
    }

    #[test]
    fn matches_simple_daily_schedule() {
        let schedule = CronSchedule::parse("30 14 * * *").unwrap();
        assert!(schedule.matches(&local_time(2026, 9, 9, 14, 30)));
        assert!(!schedule.matches(&local_time(2026, 9, 9, 14, 31)));
        assert!(!schedule.matches(&local_time(2026, 9, 9, 15, 30)));
    }

    #[test]
    fn restricted_dom_and_dow_fire_on_either() {
        // Both restricted: fires on the 1st OR on Mondays.
        let schedule = CronSchedule::parse("0 9 1 * 1").unwrap();
        // 2026-09-07 is a Monday.
        assert!(schedule.matches(&local_time(2026, 9, 7, 9, 0)));
        // 2026-09-01 is a Tuesday but is the 1st.
        assert!(schedule.matches(&local_time(2026, 9, 1, 9, 0)));
        // 2026-09-08 is a Tuesday, not the 1st.
        assert!(!schedule.matches(&local_time(2026, 9, 8, 9, 0)));
    }

    #[test]
    fn next_fire_finds_the_next_occurrence() {
        let schedule = CronSchedule::parse("0 8 * * 1-5").unwrap();
        // 2026-09-09 is a Wednesday; 8:00 already passed → next is Thursday.
        let next = schedule
            .next_fire_after(local_time(2026, 9, 9, 12, 0))
            .unwrap();
        assert_eq!(next, local_time(2026, 9, 10, 8, 0));
    }

    #[test]
    fn next_fire_skips_weekends() {
        let schedule = CronSchedule::parse("0 8 * * 1-5").unwrap();
        // 2026-09-11 is a Friday; next weekday is Monday 2026-09-14.
        let next = schedule
            .next_fire_after(local_time(2026, 9, 11, 9, 0))
            .unwrap();
        assert_eq!(next, local_time(2026, 9, 14, 8, 0));
    }

    #[test]
    fn next_fire_is_strictly_after_reference() {
        let schedule = CronSchedule::parse("* * * * *").unwrap();
        let after = local_time(2026, 9, 9, 10, 30);
        let next = schedule.next_fire_after(after).unwrap();
        assert_eq!(next, after + Duration::minutes(1));
    }

    #[test]
    fn impossible_schedule_scans_bounded_and_gives_up() {
        // February 30th never exists; the scan must terminate with None.
        let schedule = CronSchedule::parse("0 0 30 2 *").unwrap();
        assert_eq!(schedule.next_fire_after(local_time(2026, 9, 9, 0, 0)), None);
    }
}
