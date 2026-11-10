use hyper::HeaderMap;
use hyperoute_core::RemoteAddr;

/// 后台登录票据的Cookie名
pub const ADMIN_TOKEN_COOKIE: &str = "admin_token";

/// 后台登录票据的有效期(秒)
pub const ADMIN_TOKEN_MAX_AGE: i64 = 30 * 24 * 60 * 60;

/// 获取客户端IP,经过代理时优先使用代理转发的IP
pub fn get_ip(headers: &HeaderMap, addr: &RemoteAddr) -> String {
    for name in ["x-forwarded-for", "x-real-ip"] {
        if let Some(ip) = headers.get(name).and_then(|it| it.to_str().ok()) {
            //经过多层代理时,第一个才是客户端的IP
            let ip = ip.split(',').next().unwrap_or_default().trim();
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }
    addr.ip().to_string()
}

/// 从请求头中获取指定名称的Cookie值
pub fn get_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|it| it.to_str().ok())
        .flat_map(|it| it.split(';'))
        .filter_map(|it| it.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

/// 设置后台登录票据的Cookie(HttpOnly防止JS读取,只在后台路径下发送)
pub fn admin_token_cookie(token: &str) -> String {
    format!(
        "{}={}; Path=/admin; HttpOnly; SameSite=Strict; Max-Age={}",
        ADMIN_TOKEN_COOKIE, token, ADMIN_TOKEN_MAX_AGE
    )
}

/// 清除后台登录票据的Cookie
pub fn clear_admin_token_cookie() -> String {
    format!("{}=; Path=/admin; HttpOnly; SameSite=Strict; Max-Age=0", ADMIN_TOKEN_COOKIE)
}
