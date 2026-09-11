//! Pure data model and date helpers. No Cocoa, no gpui — all unit-testable.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Weekday};

/// A single calendar event, normalized away from any particular source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Event {
    /// Stable identifier from the source (EventKit identifier, etc.).
    pub id: String,
    pub title: String,
    pub location: Option<String>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub all_day: bool,
    /// The owning calendar's colour, as sRGB bytes. `None` when the source has
    /// no colour or one we cannot convert.
    pub calendar_color: Option<(u8, u8, u8)>,
    /// Free-text notes / description, trimmed; `None` when empty.
    pub notes: Option<String>,
    /// The event's own URL, if it has one.
    pub url: Option<String>,
    /// Attendee display names, the current user rendered as `"you"` and sorted
    /// last.
    pub attendees: Vec<String>,
}

/// Host fragments that mark a URL as a video-meeting link, in match order.
const MEETING_HOSTS: [&str; 5] = [
    "zoom.us",
    "meet.google.com",
    "teams.microsoft.com",
    "teams.live.com",
    "webex.com",
];

impl Event {
    /// Does this event touch `day` at all?
    pub fn occurs_on(&self, day: NaiveDate) -> bool {
        self.start.date() <= day && day <= self.end.date()
    }

    /// The link the "Join" button should open: the event's own URL when it has
    /// one, otherwise the first Zoom / Meet / Teams / Webex link found in the
    /// url, location or notes.
    pub fn join_url(&self) -> Option<String> {
        if let Some(url) = self.url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            return Some(url.to_string());
        }
        [
            self.url.as_deref(),
            self.location.as_deref(),
            self.notes.as_deref(),
        ]
        .into_iter()
        .flatten()
        .find_map(find_meeting_link)
    }

    /// `"Chetan, Priya, you"` — the collapsed attendee line, or `None` when
    /// there are no attendees.
    pub fn attendee_line(&self) -> Option<String> {
        if self.attendees.is_empty() {
            None
        } else {
            Some(self.attendees.join(", "))
        }
    }

    /// Notes cleaned up for display: real calendar invites arrive as HTML with
    /// Google's `~:~:~` separator banners in them. The raw text is kept on the
    /// event because [`Event::join_url`] scans it for meeting links.
    pub fn notes_display(&self) -> Option<String> {
        let cleaned = clean_notes(self.notes.as_deref()?);
        (!cleaned.is_empty()).then_some(cleaned)
    }
}

/// Strip HTML tags and entities, drop invite separator banners, and collapse
/// runs of blank lines.
fn clean_notes(raw: &str) -> String {
    let mut text = String::with_capacity(raw.len());
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    for (from, to) in [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
    ] {
        text = text.replace(from, to);
    }

    let mut out: Vec<&str> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        // Google's "-::~:~::~:~..." banner, and any line of pure punctuation.
        let is_banner = line.len() > 8
            && line
                .chars()
                .all(|c| matches!(c, '~' | ':' | '-' | '_' | '=' | '*' | '·'));
        if is_banner {
            continue;
        }
        if line.trim().is_empty() && out.last().is_some_and(|l| l.trim().is_empty()) {
            continue;
        }
        out.push(line);
    }
    while out.first().is_some_and(|l| l.trim().is_empty()) {
        out.remove(0);
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Order attendee names for display: everyone else alphabetically (case
/// insensitive), `"you"` last.
pub fn sort_attendees(names: &mut [String]) {
    names.sort_by(|a, b| {
        let (ay, by) = (is_you(a), is_you(b));
        ay.cmp(&by)
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
    });
}

fn is_you(name: &str) -> bool {
    name.eq_ignore_ascii_case("you")
}

/// First http(s) URL in `text` whose host looks like a video-meeting service.
fn find_meeting_link(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '<' || c == '>' || c == '"')
        .filter(|tok| tok.starts_with("http://") || tok.starts_with("https://"))
        .map(|tok| tok.trim_end_matches([',', '.', ')', ']', ';']))
        .find(|tok| {
            let lower = tok.to_lowercase();
            MEETING_HOSTS.iter().any(|h| lower.contains(h))
        })
        .map(str::to_string)
}

/// One cell of the month grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayCell {
    pub date: NaiveDate,
    /// False for the leading/trailing days that belong to the neighbouring months.
    pub in_month: bool,
}

/// One row of the month grid: seven days plus the ISO week number for that row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeekRow {
    /// ISO-8601 week number, taken from the row's Thursday.
    pub week_number: u32,
    pub days: [DayCell; 7],
}

/// The 6x7 Sunday-start grid for the month containing `anchor`.
///
/// Always six rows, so the popover never changes height between months.
pub fn month_grid(anchor: NaiveDate) -> [WeekRow; 6] {
    let first = NaiveDate::from_ymd_opt(anchor.year(), anchor.month(), 1)
        .expect("year/month from an existing date is always valid");
    // Sunday = 0 .. Saturday = 6
    let lead = first.weekday().num_days_from_sunday() as i64;
    let origin = first - Duration::days(lead);

    let month = anchor.month();
    let rows: Vec<WeekRow> = (0..6)
        .map(|row| {
            let days: [DayCell; 7] = std::array::from_fn(|col| {
                let date = origin + Duration::days(row * 7 + col as i64);
                DayCell {
                    date,
                    in_month: date.month() == month && date.year() == anchor.year(),
                }
            });
            WeekRow {
                week_number: iso_week_number(days[0].date),
                days,
            }
        })
        .collect();

    rows.try_into().expect("built exactly 6 rows")
}

/// ISO-8601 week number of the week (Sun–Sat) that starts on/contains `date`.
///
/// A Sunday-start row straddles two ISO weeks, so we take the number from the
/// row's Thursday — the same rule ISO itself uses to assign a week to a year.
pub fn iso_week_number(date: NaiveDate) -> u32 {
    let sunday = week_start(date);
    let thursday = sunday + Duration::days(4);
    thursday.iso_week().week()
}

/// The Sunday that begins the week containing `date`.
pub fn week_start(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_sunday() as i64)
}

/// The seven days, Sunday through Saturday, containing `date`.
pub fn week_containing(date: NaiveDate) -> [NaiveDate; 7] {
    let start = week_start(date);
    std::array::from_fn(|i| start + Duration::days(i as i64))
}

/// Google Calendar deep link for a single day.
pub fn google_calendar_day_url(date: NaiveDate) -> String {
    format!(
        "https://calendar.google.com/calendar/r/day/{}/{}/{}",
        date.year(),
        date.month(),
        date.day()
    )
}

/// Weekday letters for the header row, Sunday first.
pub const WEEKDAY_LETTERS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];

/// `"September 2026"`.
pub fn month_title(date: NaiveDate) -> String {
    date.format("%B %Y").to_string()
}

/// Sort order for a day's list: all-day events first, then by start time.
pub fn sort_for_day(events: &mut [Event]) {
    events.sort_by(|a, b| {
        b.all_day
            .cmp(&a.all_day)
            .then(a.start.cmp(&b.start))
            .then(a.title.cmp(&b.title))
    });
}

/// True if `date` is a weekend day.
pub fn is_weekend(date: NaiveDate) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

// ── Formatting helpers ───────────────────────────────────────────────────────

/// `"9:00"`, `"12:40"` — 12-hour clock, no zero padding, no meridiem
/// (matches `docs/mockup-popover.dc.html`).
pub fn short_time(t: NaiveTime) -> String {
    let h24 = t.hour();
    let h = match h24 % 12 {
        0 => 12,
        h => h,
    };
    format!("{}:{:02}", h, t.minute())
}

/// Title for the day list: `"Today"`, otherwise `"Thursday 11"`.
pub fn day_title(date: NaiveDate, today: NaiveDate) -> String {
    if date == today {
        "Today".to_string()
    } else {
        format!("{} {}", date.format("%A"), date.day())
    }
}

/// Right-hand summary: `"3 left"` for today, `"5 events"` otherwise,
/// empty when the day has nothing.
pub fn day_summary(count: usize, remaining: usize, is_today: bool) -> String {
    if count == 0 {
        String::new()
    } else if is_today {
        format!("{remaining} left")
    } else if count == 1 {
        "1 event".to_string()
    } else {
        format!("{count} events")
    }
}

/// Uppercase three-letter weekday for the week list: `"SUN"`.
pub fn dow_abbrev(date: NaiveDate) -> String {
    date.format("%a").to_string().to_uppercase()
}

/// Has this event already finished, relative to `now`?
pub fn is_past(end: NaiveDateTime, now: NaiveDateTime) -> bool {
    end <= now
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn grid_is_always_six_by_seven() {
        for month in 1..=12 {
            let grid = month_grid(d(2026, month, 15));
            assert_eq!(grid.len(), 6);
            for row in &grid {
                assert_eq!(row.days.len(), 7);
            }
        }
    }

    #[test]
    fn grid_starts_on_sunday_and_is_contiguous() {
        let grid = month_grid(d(2026, 9, 11));
        assert_eq!(grid[0].days[0].date.weekday(), Weekday::Sun);
        let flat: Vec<NaiveDate> = grid
            .iter()
            .flat_map(|r| r.days.iter().map(|c| c.date))
            .collect();
        let mut prev = flat[0];
        for date in flat.iter().skip(1) {
            assert_eq!(*date, prev + Duration::days(1));
            prev = *date;
        }
    }

    #[test]
    fn september_2026_leading_and_trailing_days() {
        // Sep 1 2026 is a Tuesday, so the grid starts Sun Aug 30.
        let grid = month_grid(d(2026, 9, 11));
        assert_eq!(grid[0].days[0].date, d(2026, 8, 30));
        assert!(!grid[0].days[0].in_month);
        assert_eq!(grid[0].days[2].date, d(2026, 9, 1));
        assert!(grid[0].days[2].in_month);
        // 6 rows * 7 = 42 days from Aug 30 -> last cell is Oct 10.
        assert_eq!(grid[5].days[6].date, d(2026, 10, 10));
        assert!(!grid[5].days[6].in_month);
    }

    #[test]
    fn month_starting_on_sunday_has_no_lead() {
        // Feb 1 2026 is a Sunday.
        let grid = month_grid(d(2026, 2, 20));
        assert_eq!(grid[0].days[0].date, d(2026, 2, 1));
        assert!(grid[0].days[0].in_month);
    }

    #[test]
    fn in_month_does_not_leak_across_years() {
        // January grid: the trailing cells are February, the leading ones December.
        let grid = month_grid(d(2027, 1, 5));
        let leading = grid[0].days[0];
        assert_eq!(leading.date.year(), 2026);
        assert!(!leading.in_month);
    }

    #[test]
    fn iso_week_numbers_use_the_rows_thursday() {
        // Week of Sun 2026-01-04 .. Sat 2026-01-10; Thursday is Jan 8 -> ISO week 2.
        assert_eq!(iso_week_number(d(2026, 1, 4)), 2);
        // Week of Sun 2026-09-06; Thursday Sep 10 -> ISO week 37.
        assert_eq!(iso_week_number(d(2026, 9, 6)), 37);
        // Any day in the row gives the same number.
        assert_eq!(iso_week_number(d(2026, 9, 11)), 37);
        assert_eq!(iso_week_number(d(2026, 9, 12)), 37);
    }

    #[test]
    fn grid_rows_carry_ascending_week_numbers() {
        let grid = month_grid(d(2026, 9, 11));
        let nums: Vec<u32> = grid.iter().map(|r| r.week_number).collect();
        assert_eq!(nums, vec![36, 37, 38, 39, 40, 41]);
    }

    #[test]
    fn week_start_is_sunday() {
        assert_eq!(week_start(d(2026, 9, 11)), d(2026, 9, 6));
        assert_eq!(week_start(d(2026, 9, 6)), d(2026, 9, 6));
        assert_eq!(week_start(d(2026, 9, 12)), d(2026, 9, 6));
    }

    #[test]
    fn week_containing_is_sunday_to_saturday() {
        let week = week_containing(d(2026, 9, 11));
        assert_eq!(week[0], d(2026, 9, 6));
        assert_eq!(week[6], d(2026, 9, 12));
        assert_eq!(week[0].weekday(), Weekday::Sun);
        assert_eq!(week[6].weekday(), Weekday::Sat);
    }

    #[test]
    fn google_url_has_no_zero_padding() {
        assert_eq!(
            google_calendar_day_url(d(2026, 9, 1)),
            "https://calendar.google.com/calendar/r/day/2026/9/1"
        );
        assert_eq!(
            google_calendar_day_url(d(2026, 12, 25)),
            "https://calendar.google.com/calendar/r/day/2026/12/25"
        );
    }

    #[test]
    fn month_title_formats() {
        assert_eq!(month_title(d(2026, 9, 11)), "September 2026");
    }

    #[test]
    fn all_day_events_sort_first() {
        let mk = |id: &str, h: u32, all_day: bool| Event {
            id: id.into(),
            title: id.into(),
            location: None,
            start: d(2026, 9, 11).and_hms_opt(h, 0, 0).unwrap(),
            end: d(2026, 9, 11).and_hms_opt(h + 1, 0, 0).unwrap(),
            all_day,
            ..Event::default()
        };
        let mut events = vec![
            mk("b", 14, false),
            mk("a", 9, false),
            mk("holiday", 0, true),
        ];
        sort_for_day(&mut events);
        assert_eq!(
            events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["holiday", "a", "b"]
        );
    }

    #[test]
    fn occurs_on_covers_multi_day_events() {
        let e = Event {
            id: "trip".into(),
            title: "Trip".into(),
            location: None,
            start: d(2026, 9, 10).and_hms_opt(8, 0, 0).unwrap(),
            end: d(2026, 9, 12).and_hms_opt(20, 0, 0).unwrap(),
            all_day: false,
            ..Event::default()
        };
        assert!(e.occurs_on(d(2026, 9, 10)));
        assert!(e.occurs_on(d(2026, 9, 11)));
        assert!(e.occurs_on(d(2026, 9, 12)));
        assert!(!e.occurs_on(d(2026, 9, 13)));
    }

    #[test]
    fn weekend_detection() {
        assert!(is_weekend(d(2026, 9, 12)));
        assert!(is_weekend(d(2026, 9, 13)));
        assert!(!is_weekend(d(2026, 9, 11)));
    }

    #[test]
    fn short_time_is_twelve_hour_unpadded() {
        assert_eq!(
            short_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
            "9:00"
        );
        assert_eq!(
            short_time(NaiveTime::from_hms_opt(13, 5, 0).unwrap()),
            "1:05"
        );
        assert_eq!(
            short_time(NaiveTime::from_hms_opt(0, 30, 0).unwrap()),
            "12:30"
        );
        assert_eq!(
            short_time(NaiveTime::from_hms_opt(12, 40, 0).unwrap()),
            "12:40"
        );
    }

    #[test]
    fn day_title_says_today_for_today() {
        let today = d(2026, 9, 11);
        assert_eq!(day_title(today, today), "Today");
        assert_eq!(day_title(d(2026, 9, 14), today), "Monday 14");
    }

    #[test]
    fn summaries() {
        assert_eq!(day_summary(0, 0, true), "");
        assert_eq!(day_summary(5, 3, true), "3 left");
        assert_eq!(day_summary(5, 5, false), "5 events");
        assert_eq!(day_summary(1, 1, false), "1 event");
    }

    #[test]
    fn dow_abbrev_is_upper() {
        assert_eq!(dow_abbrev(d(2026, 9, 13)), "SUN");
        assert_eq!(dow_abbrev(d(2026, 9, 11)), "FRI");
    }

    #[test]
    fn past_is_inclusive_of_the_end_instant() {
        let now = d(2026, 9, 11).and_hms_opt(12, 40, 0).unwrap();
        assert!(is_past(d(2026, 9, 11).and_hms_opt(12, 40, 0).unwrap(), now));
        assert!(!is_past(
            d(2026, 9, 11).and_hms_opt(12, 41, 0).unwrap(),
            now
        ));
    }

    fn ev_with(url: Option<&str>, location: Option<&str>, notes: Option<&str>) -> Event {
        Event {
            url: url.map(str::to_string),
            location: location.map(str::to_string),
            notes: notes.map(str::to_string),
            ..Event::default()
        }
    }

    #[test]
    fn join_url_prefers_the_events_own_url() {
        let e = ev_with(
            Some("https://example.com/agenda"),
            Some("https://zoom.us/j/123"),
            None,
        );
        assert_eq!(e.join_url().as_deref(), Some("https://example.com/agenda"));
    }

    #[test]
    fn join_url_finds_zoom_in_location() {
        let e = ev_with(None, Some("Zoom https://acme.zoom.us/j/98765?pwd=x"), None);
        assert_eq!(
            e.join_url().as_deref(),
            Some("https://acme.zoom.us/j/98765?pwd=x")
        );
    }

    #[test]
    fn join_url_finds_google_meet_in_notes() {
        let e = ev_with(
            None,
            None,
            Some("Dial in:\nhttps://meet.google.com/abc-defg-hij\nsee you there"),
        );
        assert_eq!(
            e.join_url().as_deref(),
            Some("https://meet.google.com/abc-defg-hij")
        );
    }

    #[test]
    fn join_url_finds_teams_and_webex() {
        let teams = ev_with(
            None,
            None,
            Some("Join <https://teams.microsoft.com/l/meetup-join/19%3ameeting>"),
        );
        assert_eq!(
            teams.join_url().as_deref(),
            Some("https://teams.microsoft.com/l/meetup-join/19%3ameeting")
        );
        let webex = ev_with(None, Some("https://acme.webex.com/meet/priya"), None);
        assert_eq!(
            webex.join_url().as_deref(),
            Some("https://acme.webex.com/meet/priya")
        );
    }

    #[test]
    fn join_url_is_none_without_a_meeting_link() {
        let e = ev_with(None, Some("Room 4 — Kestrel"), Some("bring the deck"));
        assert_eq!(e.join_url(), None);
        assert_eq!(Event::default().join_url(), None);
    }

    #[test]
    fn join_url_ignores_unrelated_links() {
        let e = ev_with(None, None, Some("Notes at https://notion.so/x"));
        assert_eq!(e.join_url(), None);
    }

    #[test]
    fn join_url_strips_trailing_punctuation() {
        let e = ev_with(None, None, Some("Call https://zoom.us/j/42, then debrief."));
        assert_eq!(e.join_url().as_deref(), Some("https://zoom.us/j/42"));
    }

    #[test]
    fn notes_display_strips_html_and_invite_banners() {
        let e = Event {
            notes: Some(
                "<b>Booked by</b>\nGautham &amp; co\n\n\n-::~:~::~:~::~:~::~:~::-\n~:~:~:~:~:~:~:~:~:~:~:-\nJoin with Google Meet: https://meet.google.com/foi-zmkn-gpd\n"
                    .into(),
            ),
            ..Event::default()
        };
        assert_eq!(
            e.notes_display().unwrap(),
            "Booked by\nGautham & co\n\nJoin with Google Meet: https://meet.google.com/foi-zmkn-gpd"
        );
        // The raw notes are untouched, so the join link is still found.
        assert_eq!(
            e.join_url().as_deref(),
            Some("https://meet.google.com/foi-zmkn-gpd")
        );
    }

    #[test]
    fn notes_display_is_none_when_nothing_survives() {
        let e = Event {
            notes: Some("<div></div>\n-::~:~::~:~::~:~::-\n".into()),
            ..Event::default()
        };
        assert_eq!(e.notes_display(), None);
    }

    #[test]
    fn attendees_sort_alphabetically_with_you_last() {
        let mut names = vec!["you".to_string(), "priya".to_string(), "Chetan".to_string()];
        sort_attendees(&mut names);
        assert_eq!(names, vec!["Chetan", "priya", "you"]);
    }

    #[test]
    fn attendee_line_collapses_names() {
        let e = Event {
            attendees: vec!["Chetan".into(), "Priya".into(), "you".into()],
            ..Event::default()
        };
        assert_eq!(e.attendee_line().as_deref(), Some("Chetan, Priya, you"));
        assert_eq!(Event::default().attendee_line(), None);
    }
}
