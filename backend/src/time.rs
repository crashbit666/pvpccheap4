//! Current time in Europe/Madrid.
//!
//! Every naive timestamp in the database (prices, schedules, executions) is
//! Madrid wall time. These helpers make that explicit instead of relying on
//! the container's TZ variable through `chrono::Local`.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::{Europe::Madrid, Tz};

/// Current date and time in Madrid.
pub fn now() -> DateTime<Tz> {
    Utc::now().with_timezone(&Madrid)
}

/// Current Madrid wall time, as stored in the database.
pub fn now_naive() -> NaiveDateTime {
    now().naive_local()
}

/// Today's date in Madrid.
pub fn today() -> NaiveDate {
    now().date_naive()
}

/// Start of the current hour in Madrid wall time.
pub fn current_hour() -> NaiveDateTime {
    use chrono::Timelike;
    let now = now_naive();
    now.date().and_hms_opt(now.hour(), 0, 0).unwrap()
}

/// Whether a Madrid wall time happens (the hour skipped in spring does not)
pub fn exists(t: NaiveDateTime) -> bool {
    use chrono::TimeZone;
    Madrid.from_local_datetime(&t).earliest().is_some()
}
