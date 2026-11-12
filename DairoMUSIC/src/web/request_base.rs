//! 当前请求的访问地址(协议+域名),通过DairoMUSIC代理DFS和采集服务时,用来生成客户端可以访问的完整地址
//! 由过滤器在处理请求时设置,只在处理请求的任务里可以获取

use hyper::HeaderMap;

/// 客户端访问DairoMUSIC使用的地址
#[derive(Debug, Clone)]
pub struct RequestBase {
    // http或https
    pub scheme: String,

    // 域名,可能带端口,如 example.com:8020
    pub host: String,
}

impl RequestBase {
    /// 从请求头获取(经过反向代理时优先使用X-Forwarded-Proto、X-Forwarded-Host)
    pub fn from_headers(headers: &HeaderMap) -> Option<Self> {
        let get = |name: &str| {
            headers
                .get(name)
                .and_then(|it| it.to_str().ok())
                .map(|it| it.split(',').next().unwrap_or_default().trim().to_string())
                .filter(|it| !it.is_empty())
        };
        let host = get("x-forwarded-host").or_else(|| get("host"))?;

        //只允许域名里能出现的字符,防止伪造的Host头注入到返回的链接里
        if !host.chars().all(|it| it.is_ascii_alphanumeric() || "-.:[]_".contains(it)) {
            return None;
        }
        let scheme = match get("x-forwarded-proto").as_deref() {
            Some("https") => "https",
            _ => "http",
        };
        Some(Self {
            scheme: scheme.to_string(),
            host,
        })
    }

    /// 协议+域名,如 https://example.com
    pub fn origin(&self) -> String {
        format!("{}://{}", self.scheme, self.host)
    }

    /// 不带端口的主机名
    pub fn host_name(&self) -> &str {
        if self.host.starts_with('[') {
            //IPv6地址,如 [::1]:8020
            return match self.host.find(']') {
                Some(i) => &self.host[..=i],
                None => &self.host,
            };
        }
        self.host.split(':').next().unwrap_or_default()
    }
}

tokio::task_local! {
    static CURRENT: Option<RequestBase>;
}

/// 在设置了访问地址的环境里执行
pub async fn scope<F: Future>(base: Option<RequestBase>, f: F) -> F::Output {
    CURRENT.scope(base, f).await
}

/// 当前请求的访问地址,不在处理请求的任务里时为None
pub fn current() -> Option<RequestBase> {
    CURRENT.try_with(|it| it.clone()).ok().flatten()
}
