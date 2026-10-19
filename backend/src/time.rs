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
