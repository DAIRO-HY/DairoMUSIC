//! 公开页面(分享页、下载页)用的接口,不需要登录

use crate::dao::{app_version_dao, music_dao, music_file_dao};
use crate::service::{dfs_service, software_service, vip_service};
use crate::service::music_service::{self, MusicVo};
use crate::web::biz_error::BizError;
use hyperoute_macro::GET;
use serde::Serialize;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Android版的开源仓库,最新版本从GitHub Releases获取
const ANDROID_REPO: &str = "DAIRO-HY/DairoMUSIC-Android";

/// GitHub上Android最新版本的缓存(获取时间, 下载地址和版本号),获取失败时为None
static ANDROID_LATEST: LazyLock<Mutex<Option<(Instant, Option<(String, String)>)>>> = LazyLock::new(Default::default);

/// 下载页的信息
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadInfo {
    // Android最新稳定版
    pub android_url: Option<String>,
    pub android_version: Option<String>,
}

/// 分享的歌曲
#[GET("/api/public/music")]
pub async fn music(id: i64) -> Result<MusicVo, BizError> {
    let mut ctx = lib_db::get_context();
    let Some(music) = music_dao::select_detail(&mut ctx, id).await? else {
        return Err(BizError::not_found());
    };
    let files = music_file_dao::select_by_music(&mut ctx, id).await?;
    let files: Vec<_> = files.iter().collect();
    Ok(MusicVo {
        id: music.id,
        name: music.name,
        artist: music.artist,
        album: music.album,
        logo: dfs_service::file_url(music.logo.as_deref()),
        duration: music.duration,
        video_state: music.videoState,
        files: music_service::file_vos(&files, vip_service::is_guest_limited()),
        ..Default::default()
    })
}

/// 下载页、分享页的App下载信息
/// Android优先用GitHub Releases里的最新版本,获取不到时用后台「App版本」里的最新稳定版
#[GET("/api/public/download_info")]
pub async fn download_info() -> Result<DownloadInfo, BizError> {
    if let Some((url, version)) = android_latest().await {
        return Ok(DownloadInfo {
            android_url: Some(url),
            android_version: Some(version),
        });
    }
    let mut ctx = lib_db::get_context();
    //1:Android
    let android = app_version_dao::check_update(&mut ctx, 1, 0).await?;
    Ok(DownloadInfo {
        android_url: android.as_ref().map(|it| it.url.clone()),
        android_version: android.map(|it| it.versionName),
    })
}

/// GitHub上Android最新版本的apk地址和版本号
/// 成功的结果缓存1小时,失败的缓存5分钟,避免访问量大时超过GitHub接口的次数限制
async fn android_latest() -> Option<(String, String)> {
    if let Some((time, cached)) = ANDROID_LATEST.lock().unwrap().as_ref() {
        let ttl = if cached.is_some() { 3600 } else { 300 };
        if time.elapsed() < Duration::from_secs(ttl) {
            return cached.clone();
        }
    }
    let latest = software_service::github_latest(ANDROID_REPO, |name| name.ends_with(".apk"))
        .await
        .ok()
        .filter(|it| !it.url.is_empty())
        .map(|it| (it.url, it.version));
    *ANDROID_LATEST.lock().unwrap() = Some((Instant::now(), latest.clone()));
    latest
}
