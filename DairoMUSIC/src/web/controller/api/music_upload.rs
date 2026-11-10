//! 客户端:上传本地歌曲

use crate::application;
use crate::dao::music_collect_dao::MusicCollect;
use crate::dao::music_dao::Music;
use crate::dao::{music_collect_dao, music_dao, music_file_dao};
use crate::service::collect_service;
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::filter::user_auth::LoginUser;
use crate::{biz_error, db};
use hyperoute_core::{BodyStream, Ext};
use hyperoute_macro::{GET, POST};
use rand::distr::{Alphanumeric, SampleString};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

/// 歌曲来源:会员上传
const SOURCE_USER_UPLOAD: i64 = 1;

/// 上传文件的最大大小
const MAX_SIZE: u64 = 500 * 1024 * 1024;

/// 支持上传的格式
const ALLOW_EXTS: [&str; 8] = ["mp3", "flac", "wav", "ape", "m4a", "aac", "ogg", "wma"];

/// 上传的歌曲文件保存目录(等后台转码后上传到DairoDFS)
pub fn upload_dir() -> PathBuf {
    PathBuf::from(&application::ARGS.data_dir).join("upload")
}

/// 曲库里已经有这个文件时直接返回歌曲ID,不用再上传;没有时返回404
/// - md5 文件的MD5
#[GET("/api/music/upload/by_md5")]
pub async fn by_md5(md5: String) -> Result<i64, BizError> {
    let mut ctx = lib_db::get_context();
    match music_file_dao::select_by_md5(&mut ctx, md5.to_lowercase()).await? {
        Some(it) => Ok(it.musicId),
        None => Err(BizError::not_found()),
    }
}

/// 上传歌曲文件,请求体直接放文件内容,返回歌曲ID
/// 上传后由后台转码、上传到DairoDFS,完成之前歌曲的状态为"采集中"
/// - name 文件名(带扩展名),去掉扩展名后作为歌名
#[POST("/api/music/upload")]
pub async fn upload(login: Ext<LoginUser>, name: String, mut body: BodyStream) -> Result<i64, BizError> {
    if login.is_test {
        return Err(BizError::forbidden());
    }
    let name = name.trim().to_string();
    let Some((title, ext)) = name.rsplit_once('.') else {
        return Err(BizError::field("name", "文件名需要带扩展名"));
    };
    let ext = ext.to_lowercase();
    if !ALLOW_EXTS.contains(&ext.as_str()) {
        return Err(BizError::field("name", "不支持这种格式的文件"));
    }
    if title.is_empty() || title.chars().count() > 300 {
        return Err(BizError::field("name", "文件名长度不正确"));
    }

    //先保存到上传目录,完成后再改名,中途失败时不会留下不完整的文件
    let dir = upload_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let file_name = format!("{}.{}", Alphanumeric.sample_string(&mut rand::rng(), 24), ext);
    let temp_path = dir.join(format!("{}.uploading", file_name));
    let mut file = tokio::fs::File::create(&temp_path).await?;
    let mut size = 0u64;
    while let Some(chunk) = body.next_chunk().await {
        let chunk = match chunk {
            Ok(it) => it,
            Err(e) => {
                drop(file);
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(biz_error!(format!("上传中断: {}", e)));
            }
        };
        size += chunk.len() as u64;
        if size > MAX_SIZE {
            drop(file);
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(biz_error!("文件不能超过500MB"));
        }
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);
    if size == 0 {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(biz_error!("文件内容为空"));
    }
    tokio::fs::rename(&temp_path, dir.join(&file_name)).await?;

    //添加歌曲和采集任务,由后台处理
    let now = time_util::current_millis();
    let mut ctx = db::begin().await?;
    let music = Music {
        name: Some(title.to_string()),
        sourceType: SOURCE_USER_UPLOAD,
        date: now,
        ..Default::default()
    };
    let music_id = music_dao::insert(&mut ctx, &music).await?;
    let collect = MusicCollect {
        musicId: music_id,
        extra: Some(file_name),
        platform: Some(collect_service::PLATFORM_USER_UPLOAD.to_string()),
        date: now,
        state: -1,
        updateTime: now,
        ..Default::default()
    };
    music_collect_dao::insert(&mut ctx, &collect).await?;
    ctx.commit().await?;
    crate::service::collect_task::wake();
    Ok(music_id)
}
