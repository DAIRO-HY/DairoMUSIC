//! 歌曲管理

use crate::dao::music_file_dao::MusicFile;
use crate::dao::music_collect_dao::MusicCollect;
use crate::dao::music_video_dao::MusicVideo;
use crate::dao::{music_collect_dao, music_dao, music_file_dao, music_video_dao};
use crate::dao::music_dao::Music;
use crate::service::{collect_service, dfs_service, video_task};
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::web::model::{non_empty, PageData, PageForm};
use crate::{biz_error, db, util};
use hyperoute_core::Query;
use hyperoute_macro::{GET, POST};
use rand::distr::{Alphanumeric, SampleString};
use serde::Serialize;
use validator::Validate;

/// 歌曲信息
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicItem {
    #[serde(flatten)]
    pub music: Music,

    // 封面的访问地址
    pub logo_url: Option<String>,
}

impl From<Music> for MusicItem {
    fn from(music: Music) -> Self {
        Self {
            logo_url: dfs_service::file_url(music.logo.as_deref()),
            music,
        }
    }
}

/// 歌曲列表
#[GET("/admin/api/music/list")]
pub async fn list(
    page: Query<PageForm>,
    id: Option<i64>,
    name: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    state: Option<i64>,
    delete_flag: Option<i64>,
) -> Result<PageData<MusicItem>, BizError> {
    let page_in = page.to_page_in(&["id", "name", "artist", "album", "duration", "playCount", "date", "state", "maxBitRate"], "id", "desc");
    let param = music_dao::PageListParam {
        id,
        name: non_empty(name),
        artist: non_empty(artist),
        album: non_empty(album),
        state,
        delete_flag,
    };
    let mut ctx = lib_db::get_context();
    let (total, list) = music_dao::page_list(&mut ctx, page_in, param).await?;
    Ok(PageData::new(total, list.into_iter().map(MusicItem::from).collect()))
}

/// 歌曲详情
#[GET("/admin/api/music/detail")]
pub async fn detail(id: i64) -> Result<MusicItem, BizError> {
    let mut ctx = lib_db::get_context();
    let Ok(music) = music_dao::select_one(&mut ctx, id).await else {
        return Err(biz_error!("歌曲不存在"));
    };
    Ok(music.into())
}

/// 歌曲文件
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFileItem {
    #[serde(flatten)]
    pub file: MusicFile,

    // 文件的访问地址(可以直接播放)
    pub url: Option<String>,
}

/// 歌曲的所有文件(各种音质)
#[GET("/admin/api/music/files")]
pub async fn files(id: i64) -> Result<Vec<MusicFileItem>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = music_file_dao::select_by_music(&mut ctx, id)
        .await?
        .into_iter()
        .map(|file| MusicFileItem {
            url: dfs_service::file_url(Some(&file.path)),
            file,
        })
        .collect();
    Ok(list)
}

/// 歌曲的采集记录(会员上传、采集平台的歌曲才有)
#[GET("/admin/api/music/collect")]
pub async fn collect(id: i64) -> Result<Option<MusicCollect>, BizError> {
    let mut ctx = lib_db::get_context();
    Ok(music_collect_dao::select_by_music(&mut ctx, id).await?)
}

/// 重新处理采集任务
#[POST("/admin/api/music/recollect")]
pub async fn recollect(id: i64) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    let Some(collect) = music_collect_dao::select_by_music(&mut ctx, id).await? else {
        return Err(biz_error!("这首歌没有采集记录"));
    };
    if music_collect_dao::reset(&mut ctx, collect.id, time_util::current_millis()).await? == 0 {
        return Err(biz_error!("正在处理中,请稍后再试"));
    }
    music_dao::set_collecting(&mut ctx, id).await?;
    ctx.commit().await?;
    crate::service::collect_task::wake();
    Ok(())
}

/// 视频文件
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicVideoItem {
    #[serde(flatten)]
    pub video: MusicVideo,

    // 访问地址
    pub url: Option<String>,
}

/// 歌曲的视频
#[GET("/admin/api/music/videos")]
pub async fn videos(id: i64) -> Result<Vec<MusicVideoItem>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = music_video_dao::select_by_music(&mut ctx, id)
        .await?
        .into_iter()
        .map(|video| MusicVideoItem {
            url: if video.state == 1 { dfs_service::file_url(video.path.as_deref()) } else { None },
            video,
        })
        .collect();
    Ok(list)
}

/// 删除歌曲的视频(误判为有视频时使用),以后也不再自动采集
#[POST("/admin/api/music/delete_video")]
pub async fn delete_video(id: i64) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    music_video_dao::delete_by_music(&mut ctx, id).await?;
    music_dao::set_video_state(&mut ctx, id, video_task::VIDEO_DELETED, time_util::current_millis()).await?;
    ctx.commit().await?;
    Ok(())
}

/// 重新采集歌曲的视频(包括删除了视频、判断为没有视频的歌曲)
#[POST("/admin/api/music/recollect_video")]
pub async fn recollect_video(id: i64) -> Result<(), BizError> {
    if !collect_service::is_video_enabled() {
        return Err(biz_error!("没有启用视频采集,请先在系统配置里开启"));
    }
    if video_task::processing_id() == id {
        return Err(biz_error!("正在处理中,请稍后再试"));
    }
    let mut ctx = db::begin().await?;
    let Some(collect) = music_collect_dao::select_by_music(&mut ctx, id).await? else {
        return Err(biz_error!("这首歌没有采集记录"));
    };
    if collect.platform.as_deref() != Some(collect_service::PLATFORM_YOUTUBE) {
        return Err(biz_error!("只有采集平台的歌曲才有视频"));
    }
    music_video_dao::delete_by_music(&mut ctx, id).await?;
    music_dao::set_video_state(&mut ctx, id, video_task::VIDEO_COLLECTING, time_util::current_millis()).await?;
    ctx.commit().await?;
    video_task::wake();
    Ok(())
}

/// 修改歌曲信息
#[POST("/admin/api/music/edit")]
pub async fn edit(form: form::MusicEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    if ![0, 1, 2, 3, 100].contains(&form.state) {
        return Err(BizError::field("state", "状态不正确"));
    }
    let mut ctx = db::begin().await?;
    if music_dao::select_one(&mut ctx, form.id).await.is_err() {
        return Err(biz_error!("歌曲不存在"));
    }
    let param = music_dao::UpdateByAdminParam {
        id: form.id,
        name: non_empty(Some(form.name)),
        artist: non_empty(Some(form.artist)),
        album: non_empty(Some(form.album)),
        state: form.state,
    };
    music_dao::update_by_admin(&mut ctx, param).await?;
    ctx.commit().await?;
    Ok(())
}

/// 删除歌曲(标记为删除,不删除文件)
#[POST("/admin/api/music/delete")]
pub async fn delete(id: i64) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    music_dao::set_deleted(&mut ctx, id, time_util::current_millis()).await?;
    ctx.commit().await?;
    Ok(())
}

/// 恢复已删除的歌曲
#[POST("/admin/api/music/restore")]
pub async fn restore(id: i64) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    music_dao::set_restored(&mut ctx, id).await?;
    ctx.commit().await?;
    Ok(())
}

/// 封面上传
pub struct LogoUpload {
    // 歌曲ID
    pub id: i64,

    // 图片内容
    pub file: Vec<u8>,
}

/// 上传歌曲封面(保存到DairoDFS)
#[POST("/admin/api/music/upload_logo")]
pub async fn upload_logo(form: LogoUpload) -> Result<Option<String>, BizError> {
    let Some(ext) = util::image_util::image_ext(&form.file) else {
        return Err(biz_error!("只支持JPG、PNG、WEBP格式的图片"));
    };
    if form.file.len() > 10 * 1024 * 1024 {
        return Err(biz_error!("图片不能超过10MB"));
    }
    let mut ctx = lib_db::get_context();
    if music_dao::select_one(&mut ctx, form.id).await.is_err() {
        return Err(biz_error!("歌曲不存在"));
    }
    let path = logo_path(ext);
    if let Err(e) = dfs_service::upload(form.file, &path, "").await {
        return Err(biz_error!(format!("封面上传失败: {}", e)));
    }
    music_dao::update_logo(&mut ctx, form.id, Some(path.clone())).await?;
    Ok(dfs_service::file_url(Some(&path)))
}

/// 生成封面的存储路径 /image/logo/{年}/{随机名}.{扩展名}
fn logo_path(ext: &str) -> String {
    let year = time_util::current_year();
    let name = Alphanumeric.sample_string(&mut rand::rng(), 16);
    format!("/image/logo/{}/{}.{}", year, name, ext)
}
