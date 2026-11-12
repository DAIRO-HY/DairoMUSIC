//! 采集平台(目前只有YouTube采集服务)

use crate::service::config_service;
use crate::web::request_base;
use serde::Deserialize;
use std::sync::LazyLock;
use std::time::Duration;

/// 采集平台:YouTube
pub const PLATFORM_YOUTUBE: &str = "YT";

/// 采集平台:会员上传
pub const PLATFORM_USER_UPLOAD: &str = "UU";

/// 代理采集服务的路径前缀
pub const PROXY_PREFIX: &str = "/proxy/dairo-search";

/// HTTP客户端
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
});

/// 采集平台的搜索结果
#[derive(Debug)]
pub struct SearchItem {
    pub name: String,
    pub logo: Option<String>,
    pub platform: String,
    pub extra: String,
}

/// YouTube采集服务返回的搜索结果
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeItem {
    #[serde(default)]
    name: String,
    logo: Option<String>,
    #[serde(default)]
    video_id: String,
}

/// 是否启用了采集服务
pub fn is_enabled() -> bool {
    let config = config_service::get();
    config.collect_enable && (config.collect_builtin || !config.collect_url.is_empty())
}

/// 采集服务的地址(服务端调用),内置采集服务使用本机地址
pub fn api_url() -> String {
    let config = config_service::get();
    if config.collect_builtin {
        format!("http://127.0.0.1:{}", config_service::collect_port(&config))
    } else {
        config.collect_url.trim().trim_end_matches('/').to_string()
    }
}

/// 测试能否连接采集服务
pub async fn test() -> Result<String, String> {
    let url = format!("{}/d/collect_info?videoId=test", api_url());
    match CLIENT.get(url).send().await {
        Ok(_) => Ok("连接成功".to_string()),
        Err(e) => Err(format!("连接失败: {}", e)),
    }
}

/// 上传YouTube的cookie(Netscape格式),内置采集服务直接写文件,外部的调用采集服务的接口
pub async fn set_cookie(cookie: &str) -> Result<(), String> {
    let config = config_service::get();
    if config.collect_builtin {
        let dir = config_service::collect_data_dir(&config).join("data");
        tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
        return tokio::fs::write(dir.join("youtube-cookie.txt"), cookie).await.map_err(|e| e.to_string());
    }
    let res = CLIENT
        .post(format!("{}/set/cookie", api_url()))
        .form(&[("cookie", cookie)])
        .send()
        .await
        .map_err(|e| format!("连接采集服务失败: {}", e))?;
    if !res.status().is_success() {
        return Err(format!("采集服务返回错误: {}", res.status()));
    }
    Ok(())
}

/// 从采集平台搜索
pub async fn search(key: &str) -> Result<Vec<SearchItem>, String> {
    if !is_enabled() {
        return Ok(Vec::new());
    }
    let url = format!("{}/search/api", api_url());
    let res = CLIENT.get(url).query(&[("key", key)]).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("采集平台搜索失败: {}", res.status()));
    }
    let list: Vec<YoutubeItem> = res.json().await.map_err(|e| e.to_string())?;
    let list = list
        .into_iter()
        .filter(|it| !it.video_id.is_empty())
        .map(|it| SearchItem {
            //YouTube上的歌名大部分是繁体,转换成简体
            name: zhconv::zhconv(&it.name, zhconv::Variant::ZhCN),
            //采集服务返回的封面地址是它自己的地址(内置时是127.0.0.1),换成客户端可以访问的地址
            logo: it.logo.filter(|it| !it.is_empty()).map(|it| to_public_url(&it)),
            platform: PLATFORM_YOUTUBE.to_string(),
            extra: it.video_id,
        })
        .collect();
    Ok(list)
}

/// 采集平台的播放地址(客户端直接访问)
pub fn play_url(platform: &str, extra: &str) -> Option<String> {
    if platform != PLATFORM_YOUTUBE || !is_enabled() {
        return None;
    }
    Some(format!("{}/d/music?videoId={}", public_url(), extra))
}

/// 采集平台的视频预览地址(没有收藏的歌曲,客户端直接播放;iPhone能直接播放的编码)
pub fn video_preview_url(platform: &str, extra: &str) -> Option<String> {
    if platform != PLATFORM_YOUTUBE || !is_video_enabled() {
        return None;
    }
    Some(format!("{}/d/video?videoId={}&type=preview", public_url(), extra))
}

/// 采集平台的视频预览采集进度地址(返回OK时可以播放)
pub fn video_preview_info_url(platform: &str, extra: &str) -> Option<String> {
    if platform != PLATFORM_YOUTUBE || !is_video_enabled() {
        return None;
    }
    Some(format!("{}/d/video_info?videoId={}&type=preview", public_url(), extra))
}

/// 是否采集视频
pub fn is_video_enabled() -> bool {
    is_enabled() && config_service::get().video_enable
}

/// 采集服务的地址换成客户端可以访问的地址
fn to_public_url(url: &str) -> String {
    let api = api_url();
    match url.strip_prefix(&api) {
        Some(rest) if !api.is_empty() && rest.starts_with('/') => format!("{}{}", public_url(), rest),
        _ => url.to_string(),
    }
}

/// 客户端访问的地址换回采集服务的地址(服务端下载封面时使用,不用绕到外网再回来)
pub fn to_internal_url(url: &str) -> String {
    if let Some(i) = url.find(PROXY_PREFIX) {
        let rest = &url[i + PROXY_PREFIX.len()..];
        if rest.starts_with("/d/") {
            return format!("{}{}", api_url(), rest);
        }
    }
    url.to_string()
}

/// 采集平台对外的地址(客户端访问)
fn public_url() -> String {
    let config = config_service::get();
    if config.collect_public_mode != "direct" {
        //通过DairoMUSIC代理,和访问DairoMUSIC使用同一个域名
        let base = request_base::current().map(|it| it.origin()).unwrap_or_default();
        return format!("{}{}", base, PROXY_PREFIX);
    }
    let url = config.collect_public_url.trim().trim_end_matches('/');
    if !url.is_empty() {
        return url.to_string();
    }
    if config.collect_builtin {
        //内置采集服务:客户端访问DairoMUSIC的主机名 + 采集服务的端口
        let port = config_service::collect_port(&config);
        return match request_base::current() {
            Some(it) => format!("{}://{}:{}", it.scheme, it.host_name(), port),
            None => format!("http://127.0.0.1:{}", port),
        };
    }
    api_url()
}
