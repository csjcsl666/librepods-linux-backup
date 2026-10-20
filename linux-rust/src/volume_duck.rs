use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Conversational awareness lowers the output volume while someone is talking. The system volume
// is persisted by WirePlumber, so a lowered volume that is never restored (exit, crash, reconnect)
// stays lowered. The session is kept in a state file so it can be restored on the next start, and
// a restore never overwrites a volume the user changed in the meantime.

const RESTORE_TOLERANCE: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuckSession {
    pub sink: String,
    pub original: u32,
    pub last_set: u32,
}

fn close(a: u32, b: u32) -> bool {
    a.abs_diff(b) <= RESTORE_TOLERANCE
}

// Decide the volume to set for a conversation level, updating the session.
// A session only starts when `may_start` is set and the current volume is above the target.
// An unreadable volume never starts a session and is never treated as 0.
pub fn plan_duck(
    current: Option<u32>,
    session: &mut Option<DuckSession>,
    sink: &str,
    target: u32,
    may_start: bool,
) -> Option<u32> {
    let current = current?;
    match session {
        None => {
            if may_start && current > target {
                *session = Some(DuckSession {
                    sink: sink.to_string(),
                    original: current,
                    last_set: target,
                });
                Some(target)
            } else {
                None
            }
        }
        Some(s) => {
            if !close(current, s.last_set) {
                // the user changed the volume, their value wins and the session is over
                *session = None;
                return None;
            }
            let new = target.min(s.original);
            if close(new, current) {
                s.last_set = new;
                return None;
            }
            s.last_set = new;
            Some(new)
        }
    }
}

// Volume to restore to, or None when nothing should be touched. The session is consumed unless
// the volume cannot be read.
pub fn plan_restore(current: Option<u32>, session: &mut Option<DuckSession>) -> Option<u32> {
    // the sink is gone (disconnected), keep the session so a later attempt can still restore it
    let current = current?;
    let s = session.take()?;
    if close(current, s.last_set) && !close(current, s.original) {
        Some(s.original)
    } else {
        None
    }
}

fn state_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("librepods").join("volume-duck.json"))
}

pub fn save(session: &Option<DuckSession>) {
    let Some(path) = state_path() else { return };
    match session {
        Some(s) => {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match serde_json::to_string(s) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(&path, json) {
                        warn!("Failed to save volume state: {}", e);
                    }
                }
                Err(e) => warn!("Failed to serialize volume state: {}", e),
            }
        }
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
}

pub fn load() -> Option<DuckSession> {
    let json = std::fs::read_to_string(state_path()?).ok()?;
    serde_json::from_str(&json).ok()
}

// Restore a session left behind by a previous run, exit or disconnect. Safe to call at any time.
pub fn restore_blocking() {
    let mut session = load();
    let Some(s) = session.clone() else { return };
    let current = crate::media_controller::get_sink_volume_percent_by_name_sync(&s.sink);
    if current.is_none() {
        info!("Sink {} not available yet, keeping the saved volume state", s.sink);
        return;
    }
    match plan_restore(current, &mut session) {
        Some(volume) => {
            info!("Restoring volume of {} to {}% after conversational awareness", s.sink, volume);
            crate::media_controller::transition_sink_volume(&s.sink, volume);
        }
        None => info!("Volume of {} was changed by the user, leaving it as it is", s.sink),
    }
    save(&None);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINK: &str = "bluez_output.test";

    fn started(original: u32, last_set: u32) -> Option<DuckSession> {
        Some(DuckSession { sink: SINK.into(), original, last_set })
    }

    #[test]
    fn starts_and_lowers_once() {
        let mut s = None;
        assert_eq!(plan_duck(Some(75), &mut s, SINK, 25, true), Some(25));
        assert_eq!(s, started(75, 25));
    }

    #[test]
    fn does_not_start_below_target() {
        let mut s = None;
        assert_eq!(plan_duck(Some(20), &mut s, SINK, 25, true), None);
        assert_eq!(s, None);
    }

    #[test]
    fn unreadable_volume_never_starts_or_zeroes() {
        let mut s = None;
        assert_eq!(plan_duck(None, &mut s, SINK, 25, true), None);
        assert_eq!(s, None);
        let mut s = started(75, 25);
        assert_eq!(plan_restore(None, &mut s), None);
        assert_eq!(s, started(75, 25));
    }

    #[test]
    fn reduce_without_session_is_ignored() {
        let mut s = None;
        assert_eq!(plan_duck(Some(75), &mut s, SINK, 15, false), None);
    }

    #[test]
    fn deeper_then_partial_increase_is_capped_by_original() {
        let mut s = started(75, 25);
        assert_eq!(plan_duck(Some(25), &mut s, SINK, 15, false), Some(15));
        assert_eq!(plan_duck(Some(15), &mut s, SINK, 25, false), Some(25));
        let mut s = started(20, 20);
        assert_eq!(plan_duck(Some(20), &mut s, SINK, 25, false), None);
    }

    #[test]
    fn user_change_ends_session_without_overwrite() {
        let mut s = started(75, 25);
        assert_eq!(plan_duck(Some(60), &mut s, SINK, 15, false), None);
        assert_eq!(s, None);
    }

    #[test]
    fn restore_returns_original() {
        let mut s = started(75, 25);
        assert_eq!(plan_restore(Some(25), &mut s), Some(75));
        assert_eq!(s, None);
    }

    #[test]
    fn restore_after_crash_uses_persisted_values() {
        let json = serde_json::to_string(&started(75, 15).unwrap()).unwrap();
        let mut s: Option<DuckSession> = serde_json::from_str(&json).ok();
        assert_eq!(plan_restore(Some(15), &mut s), Some(75));
    }

    #[test]
    fn restore_never_overwrites_user_volume() {
        let mut s = started(75, 25);
        assert_eq!(plan_restore(Some(40), &mut s), None);
        assert_eq!(s, None);
    }

    #[test]
    fn restore_skips_when_already_original() {
        let mut s = started(75, 25);
        assert_eq!(plan_restore(Some(75), &mut s), None);
    }

    #[test]
    fn rounding_tolerance_is_one_percent() {
        let mut s = started(75, 25);
        assert_eq!(plan_restore(Some(26), &mut s), Some(75));
        let mut s = started(75, 25);
        assert_eq!(plan_restore(Some(28), &mut s), None);
    }
}
