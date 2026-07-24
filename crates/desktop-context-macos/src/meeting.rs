//! Pure meeting process-name matching (no titles or content inspection).

use crate::types::{MeetingFact, MeetingHint};

/// Returns a meeting hint when `name` matches a known conferencing app.
///
/// Matching is case-insensitive substring against process/app display names only.
#[must_use]
pub fn meeting_hint_from_process_name(name: &str) -> MeetingHint {
    let lower = name.to_ascii_lowercase();
    if lower.contains("zoom") {
        return MeetingHint::Zoom;
    }
    if lower.contains("teams") || lower.contains("ms-teams") {
        return MeetingHint::Teams;
    }
    // Google Meet: require explicit markers (avoid bare "Meetings" false positives).
    if lower.contains("google meet")
        || lower.contains("googlemeet")
        || (lower.contains("meet") && lower.contains("chrome"))
    {
        return MeetingHint::Meet;
    }
    if lower.contains("meeting center") || lower.contains("webex") {
        return MeetingHint::Webex;
    }
    // FaceTime is a conferencing signal on macOS (privacy: name only).
    if lower.contains("facetime") {
        return MeetingHint::Unknown;
    }
    MeetingHint::None
}

/// Scans process/app names and returns the strongest meeting fact found.
///
/// Priority: Zoom > Teams > Meet > Webex. Only explicit markers produce active meetings.
#[must_use]
pub fn meeting_from_process_names<'a, I>(names: I) -> MeetingFact
where
    I: IntoIterator<Item = &'a str>,
{
    let mut best = MeetingHint::None;
    for name in names {
        let hint = meeting_hint_from_process_name(name);
        best = prefer_meeting_hint(best, hint);
    }
    if best == MeetingHint::None {
        MeetingFact::inactive()
    } else {
        MeetingFact {
            active: true,
            hint: best,
        }
    }
}

/// Prefer a more specific meeting hint over a weaker one.
#[must_use]
pub const fn prefer_meeting_hint(current: MeetingHint, next: MeetingHint) -> MeetingHint {
    match (current, next) {
        (_, MeetingHint::None) => current,
        (MeetingHint::None, other) => other,
        (MeetingHint::Zoom, _) | (_, MeetingHint::Zoom) => MeetingHint::Zoom,
        (MeetingHint::Teams, _) | (_, MeetingHint::Teams) => MeetingHint::Teams,
        (MeetingHint::Meet, _) | (_, MeetingHint::Meet) => MeetingHint::Meet,
        (MeetingHint::Webex, _) | (_, MeetingHint::Webex) => MeetingHint::Webex,
        (MeetingHint::Unknown, other) => other,
    }
}
