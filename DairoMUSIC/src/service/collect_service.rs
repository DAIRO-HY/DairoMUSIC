//! 采集平台(目前只有YouTube采集服务)

use crate::service::config_service;
use serde::Deserialize;
use std::sync::LazyLock;
use std::time::Duration;

/// 采集平台:YouTube
pub const PLATFORM_YOUTUBE: &str = "YT";

/// 采集平台:会员上传
pub const PLATFORM_USER_UPLOAD: &str = "UU";

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
    config.collect_enable && !config.collect_url.is_empty()
}

/// 采集服务的地址(服务端调用)
pub fn api_url() -> String {
    config_service::get().collect_url.trim_end_matches('/').to_string()
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
            logo: it.logo.filter(|it| !it.is_empty()),
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

/// 采集平台对外的地址
fn public_url() -> String {
    let config = config_service::get();
    let url = if config.collect_public_url.is_empty() {
        config.collect_url
    } else {
        config.collect_public_url
    };
    url.trim_end_matches('/').to_string()
}
