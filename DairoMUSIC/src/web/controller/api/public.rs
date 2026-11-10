//! 公开页面(分享页、下载页)用的接口,不需要登录

use crate::dao::{app_version_dao, music_dao, music_file_dao};
use crate::service::dfs_service;
use crate::service::music_service::{self, MusicVo};
use crate::web::biz_error::BizError;
use hyperoute_macro::GET;
use serde::Serialize;

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
        files: music_service::file_vos(&files),
        ..Default::default()
    })
}

/// 下载页的信息
#[GET("/api/public/download_info")]
pub async fn download_info() -> Result<DownloadInfo, BizError> {
    let mut ctx = lib_db::get_context();
    //1:Android
    let android = app_version_dao::check_update(&mut ctx, 1, 0).await?;
    Ok(DownloadInfo {
        android_url: android.as_ref().map(|it| it.url.clone()),
        android_version: android.map(|it| it.versionName),
    })
}
