use std::time::{SystemTime, UNIX_EPOCH};

/// 当前时间戳(毫秒)
pub fn current_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|it| it.as_millis() as i64)
        .unwrap_or(0)
}

/// 日期(如 2026-11-14,本地时区)那一天的最后一毫秒,格式不正确时返回None
pub fn date_end_millis(date: &str) -> Option<i64> {
    use chrono::TimeZone;
    let date = chrono::NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
    let time = date.and_hms_milli_opt(23, 59, 59, 999)?;
    chrono::Local.from_local_datetime(&time).earliest().map(|it| it.timestamp_millis())
}

/// 时间戳(毫秒)的日期(本地时区),如 2026-11-14
pub fn date_text(millis: i64) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_millis_opt(millis).earliest().map(|it| it.format("%Y-%m-%d").to_string()).unwrap_or_default()
}

/// 当前年份(本地时区)
pub fn current_year() -> i32 {
    use chrono::Datelike;
    chrono::Local::now().year()
}

/// 当前时间的显示文本(本地时区),如 11-12 21:37:05
pub fn now_text() -> String {
    chrono::Local::now().format("%m-%d %H:%M:%S").to_string()
}
