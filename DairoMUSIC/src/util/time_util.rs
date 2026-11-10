use std::time::{SystemTime, UNIX_EPOCH};

/// 当前时间戳(毫秒)
pub fn current_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|it| it.as_millis() as i64)
        .unwrap_or(0)
}

/// 当前年份(本地时区)
pub fn current_year() -> i32 {
    use chrono::Datelike;
    chrono::Local::now().year()
}
