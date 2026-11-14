//! 歌曲管理

use crate::dao::music_file_dao::MusicFile;
use crate::dao::music_collect_dao::MusicCollect;
use crate::dao::music_video_dao::MusicVideo;
use crate::dao::music_lrc_dao::MusicLrc;
use crate::dao::{music_collect_dao, music_dao, music_file_dao, music_lrc_dao, music_video_dao};
use crate::dao::music_dao::Music;
use crate::service::{collect_service, dfs_service, video_task};
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::web::model::{non_empty, PageData, PageForm};
use crate::{biz_error, biz_errorf, db};
use rand::distr::{Alphanumeric, SampleString};
use tokio::io::AsyncWriteExt;
use crate::web::controller::api::music_upload;
use hyperoute_core::{BodyStream, Query};
use hyperoute_macro::{GET, POST};
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
    collect_state: Option<i64>,
    audit_state: Option<i64>,
    delete_flag: Option<i64>,
) -> Result<PageData<MusicItem>, BizError> {
    let page_in = page.to_page_in(&["id", "name", "artist", "album", "duration", "playCount", "date", "collectState", "auditState", "maxBitRate"], "id", "desc");
    let param = music_dao::PageListParam {
        id,
        name: non_empty(name),
        artist: non_empty(artist),
        album: non_empty(album),
        collect_state,
        audit_state,
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
    //还没有处理的上传视频也一起删除
    video_task::remove_upload(id).await;
    let mut ctx = db::begin().await?;
    music_video_dao::delete_by_music(&mut ctx, id).await?;
    music_dao::set_video_state(&mut ctx, id, video_task::VIDEO_DELETED, time_util::current_millis()).await?;
    ctx.commit().await?;
    Ok(())
}

/// 手动上传歌曲的视频,请求体直接放文件内容;所有歌曲都可以上传,已经有视频时替换原来的
/// 上传后由后台转码(和采集的视频一样:原画、流畅版)、上传到DairoDFS,完成之前视频状态为"处理中"
/// - id 歌曲ID
/// - name 文件名(带扩展名)
#[POST("/admin/api/music/upload_video")]
pub async fn upload_video(id: i64, name: String, mut body: BodyStream) -> Result<(), BizError> {
    let ext = name.rsplit_once('.').map(|(_, ext)| ext.to_lowercase()).unwrap_or_default();
    if !video_task::UPLOAD_EXTS.contains(&ext.as_str()) {
        return Err(biz_errorf!("只支持这些格式的视频: {}", video_task::UPLOAD_EXTS.join("、")));
    }
    let mut ctx = lib_db::get_context();
    if !music_dao::select_one(&mut ctx, id).await.is_ok_and(|it| it.deleteFlag == 0) {
        return Err(biz_error!("歌曲不存在"));
    }
    if video_task::processing_id() == id {
        return Err(biz_error!("这首歌的视频正在处理中,请稍后再试"));
    }

    //先保存成临时文件,上传完成后再改名,后台任务不会处理还没有上传完的文件
    let dir = video_task::upload_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let temp_path = dir.join(format!("{}.{}.uploading", id, Alphanumeric.sample_string(&mut rand::rng(), 8)));
    let mut file = tokio::fs::File::create(&temp_path).await?;
    let mut size = 0u64;
    while let Some(chunk) = body.next_chunk().await {
        let error = match chunk {
            Ok(chunk) => {
                size += chunk.len() as u64;
                if size > video_task::UPLOAD_MAX_SIZE {
                    Some(biz_error!("视频不能超过20GB"))
                } else {
                    file.write_all(&chunk).await.err().map(BizError::from)
                }
            }
            Err(e) => Some(biz_errorf!("上传中断: {}", e)),
        };
        if let Some(error) = error {
            drop(file);
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(error);
        }
    }
    file.flush().await?;
    drop(file);
    if size == 0 || video_task::processing_id() == id {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(biz_error!(if size == 0 { "文件内容为空" } else { "这首歌的视频正在处理中,请稍后再试" }));
    }

    //同一首歌只保留最后上传的
    video_task::remove_upload(id).await;
    tokio::fs::rename(&temp_path, dir.join(format!("{}.{}", id, ext))).await?;
    let mut ctx = db::begin().await?;
    music_video_dao::delete_by_music(&mut ctx, id).await?;
    music_dao::set_video_state(&mut ctx, id, video_task::VIDEO_COLLECTING, time_util::current_millis()).await?;
    ctx.commit().await?;
    video_task::wake();
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

    //手动上传了但还没有处理的视频不要了
    video_task::remove_upload(id).await;
    video_task::wake();
    Ok(())
}

/// 修改歌曲信息
#[POST("/admin/api/music/edit")]
pub async fn edit(form: form::MusicEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let mut ctx = db::begin().await?;
    if music_dao::select_one(&mut ctx, form.id).await.is_err() {
        return Err(biz_error!("歌曲不存在"));
    }
    music_dao::update_by_admin(&mut ctx, form.id, non_empty(Some(form.name)), non_empty(Some(form.artist)), non_empty(Some(form.album))).await?;
    ctx.commit().await?;
    Ok(())
}

/// 歌曲的歌词(一首歌可以有多个歌词,APP里审核通过的排在前面)
#[GET("/admin/api/music/lrcs")]
pub async fn lrcs(id: i64) -> Result<Vec<MusicLrc>, BizError> {
    let mut ctx = lib_db::get_context();
    Ok(music_lrc_dao::select_by_music(&mut ctx, id).await?)
}

/// 添加、修改歌词
#[POST("/admin/api/music/lrc_save")]
pub async fn lrc_save(form: form::MusicLrcEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    if form.lrc.trim().is_empty() {
        return Err(BizError::field("lrc", "歌词内容不能为空"));
    }
    if ![0, 1].contains(&form.state) {
        return Err(BizError::field("state", "状态不正确"));
    }
    let title = form.title.trim();
    let title = if title.is_empty() { "后台添加" } else { title }.to_string();
    let mut ctx = db::begin().await?;
    if form.id == 0 {
        if music_dao::select_one(&mut ctx, form.music_id).await.is_err() {
            return Err(biz_error!("歌曲不存在"));
        }
        let lrc = MusicLrc {
            musicId: form.music_id,
            title,
            lrc: form.lrc,
            state: form.state,
            ..Default::default()
        };
        music_lrc_dao::insert(&mut ctx, &lrc).await?;
    } else {
        //只能修改这首歌的歌词
        let mut lrc = match music_lrc_dao::select_one(&mut ctx, form.id).await {
            Ok(it) if it.musicId == form.music_id => it,
            _ => return Err(biz_error!("歌词不存在")),
        };
        lrc.title = title;
        lrc.lrc = form.lrc;
        lrc.state = form.state;
        music_lrc_dao::update(&mut ctx, &lrc).await?;
    }
    ctx.commit().await?;
    Ok(())
}

/// 删除歌词
#[POST("/admin/api/music/lrc_delete")]
pub async fn lrc_delete(id: i64) -> Result<(), BizError> {
    let mut ctx = lib_db::get_context();
    music_lrc_dao::delete(&mut ctx, id).await?;
    Ok(())
}

/// 审核状态 0:待审核 1:通过 2:拒绝(APP搜索不显示)
const AUDIT_STATES: [i64; 3] = [0, 1, 2];

/// 批量审核
#[POST("/admin/api/music/audit")]
pub async fn audit(form: form::MusicAudit) -> Result<(), BizError> {
    if !AUDIT_STATES.contains(&form.audit_state) {
        return Err(biz_error!("审核状态不正确"));
    }
    let mut ids = Vec::new();
    for it in form.ids.split(',').map(str::trim).filter(|it| !it.is_empty()) {
        let Ok(id) = it.parse::<i64>() else {
            return Err(biz_error!("歌曲ID格式错误"));
        };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    //没有ID时会修改所有歌曲,必须检查
    if ids.is_empty() {
        return Err(biz_error!("请选择要审核的歌曲"));
    }
    if ids.len() > 1000 {
        return Err(biz_error!("一次最多审核1000首歌曲"));
    }
    let mut ctx = lib_db::get_context();
    music_dao::set_audit_state(&mut ctx, ids, form.audit_state).await?;
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
/// 不限制上传的图片大小,上传后居中裁切成正方形并缩小到800x800,转换成JPG
#[POST("/admin/api/music/upload_logo")]
pub async fn upload_logo(form: LogoUpload) -> Result<Option<String>, BizError> {
    let mut ctx = lib_db::get_context();
    if music_dao::select_one(&mut ctx, form.id).await.is_err() {
        return Err(biz_error!("歌曲不存在"));
    }
    let path = crate::web::controller::api::user::upload_image(form.file).await?;
    music_dao::update_logo(&mut ctx, form.id, Some(path.clone())).await?;
    Ok(dfs_service::file_url(Some(&path)))
}

/// 上传歌曲文件,请求体直接放文件内容,返回歌曲ID
/// 和APP上传歌曲一样由后台转码、上传到DairoDFS,完成之前歌曲的状态为"采集中";曲库里已经有相同的文件时自动合并
/// - name 文件名(带扩展名),去掉扩展名后作为歌名
#[POST("/admin/api/music/upload")]
pub async fn upload(name: String, body: BodyStream) -> Result<i64, BizError> {
    music_upload::save_upload(name, body, music_upload::SOURCE_ADMIN_UPLOAD).await
}
