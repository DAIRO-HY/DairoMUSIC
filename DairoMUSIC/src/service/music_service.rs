//! 返回给客户端的歌曲数据

use crate::dao::music_file_dao::MusicFile;
use crate::service::{collect_service, dfs_service};
use serde::Serialize;
use std::collections::HashMap;

/// 歌曲
#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicVo {
    // 歌曲ID,采集平台搜索结果(还没有添加到曲库)为0
    pub id: i64,
    pub name: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,

    // 封面地址
    pub logo: Option<String>,

    // 时长(毫秒)
    pub duration: Option<i64>,

    // 所属歌单ID(歌单里的歌曲才有)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite_id: Option<i64>,

    // 视频状态 0:未采集 1:采集中 2:有视频 3:没有视频(只有一张图片) 4:后台删除了视频 5:采集失败
    pub video_state: i64,

    // 播放次数(只有曲库的搜索结果才有)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub play_count: Option<i64>,

    // 审核状态 0:待审核 1:通过 2:拒绝(只有信任会员获取歌曲详情时才有)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_state: Option<i64>,

    // 所有音质的文件,按比特率从低到高排序,还没有采集完成时为空数组
    pub files: Vec<FileVo>,

    // 采集信息,文件为空时可以用采集平台的地址播放
    pub collect: Option<CollectVo>,
}

/// 歌曲文件
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileVo {
    // 文件ID(不会改变,客户端可以用来做缓存的key)
    pub id: i64,

    // 文件的MD5,老数据可能为空
    pub md5: Option<String>,

    pub bit_rate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub data_start_position: Option<i64>,
    pub sample_count: Option<i64>,
    pub frame_length: Option<i64>,
    pub size: i64,
    pub format: String,

    // 播放地址;只有VIP才能听的文件(vip_only为true)没有地址
    pub url: Option<String>,

    // 是否只有VIP才能听(开启了VIP功能,并且当前会员不是VIP时,除了最低音质的文件都是true)
    pub vip_only: bool,
}

/// 采集信息
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CollectVo {
    // 采集平台
    pub platform: String,

    // 采集参数
    pub extra: String,

    // 采集平台的播放地址
    pub url: Option<String>,

    // 采集平台的视频预览地址(还没有收藏的歌曲直接播放,mp4)
    pub video_url: Option<String>,

    // 采集平台的视频预览采集进度地址,返回OK时可以播放
    pub video_info_url: Option<String>,
}

impl From<&MusicFile> for FileVo {
    fn from(it: &MusicFile) -> Self {
        Self {
            id: it.id,
            md5: it.md5.clone(),
            bit_rate: it.bitRate,
            sample_rate: it.sampleRate,
            data_start_position: it.dataStartPosition,
            sample_count: it.sampleCount,
            frame_length: it.frameLength,
            size: it.size,
            format: it.format.clone(),
            url: dfs_service::file_url(Some(&it.path)),
            vip_only: false,
        }
    }
}

/// 生成采集信息
pub fn collect_vo(platform: Option<String>, extra: Option<String>) -> Option<CollectVo> {
    let (platform, extra) = (platform?, extra?);
    Some(CollectVo {
        url: collect_service::play_url(&platform, &extra),
        video_url: collect_service::video_preview_url(&platform, &extra),
        video_info_url: collect_service::video_preview_info_url(&platform, &extra),
        platform,
        extra,
    })
}

/// 转换成返回给客户端的文件列表,按比特率从低到高排序
/// - vip_limited 是否受到非VIP的限制:只有最低音质(标准音质)的文件有播放地址,其他文件标记为只有VIP才能听
///   (文件信息还是返回,VIP到期之后客户端可以继续播放以前缓存的文件)
pub fn file_vos(files: &[&MusicFile], vip_limited: bool) -> Vec<FileVo> {
    let mut files = files.to_vec();
    files.sort_by_key(|it| (it.bitRate.unwrap_or(0), it.id));
    let mut list: Vec<FileVo> = files.into_iter().map(Into::into).collect();
    if vip_limited {
        for it in list.iter_mut().skip(1) {
            it.url = None;
            it.vip_only = true;
        }
    }
    list
}

/// 按歌曲ID分组
pub fn group_files(files: &[MusicFile]) -> HashMap<i64, Vec<&MusicFile>> {
    let mut map: HashMap<i64, Vec<&MusicFile>> = HashMap::new();
    for it in files {
        map.entry(it.musicId).or_default().push(it);
    }
    map
}

/// 某首歌曲的文件列表
pub fn files_of(file_map: &HashMap<i64, Vec<&MusicFile>>, music_id: i64, vip_limited: bool) -> Vec<FileVo> {
    file_map.get(&music_id).map(|files| file_vos(files, vip_limited)).unwrap_or_default()
}

/// 把几首歌曲合并到保留的歌曲:收藏了这些歌曲的歌单改为收藏保留的歌曲,这些歌曲标记为删除
/// 保留的歌曲没有封面时,使用被合并的歌曲的封面
/// - ids 要合并的歌曲ID(不包括保留的歌曲)
/// - best_id 保留的歌曲ID
pub async fn merge(ctx: &mut sqlx_context::DbContext, ids: &[i64], best_id: i64) -> Result<(), crate::web::biz_error::BizError> {
    use crate::dao::{music_dao, user_favorite_dao, user_music_dao};
    use crate::util::time_util;
    use std::collections::BTreeSet;

    let now = time_util::current_millis();
    let Ok(best) = music_dao::select_one(ctx, best_id).await else {
        return Err(crate::biz_error!("保留的歌曲不存在"));
    };
    let mut logo = best.logo.clone();
    let mut favorite_ids = BTreeSet::new();
    for &id in ids {
        if id == best_id {
            continue;
        }
        let Ok(music) = music_dao::select_one(ctx, id).await else {
            return Err(crate::biz_error!(format!("歌曲{}不存在", id)));
        };
        if logo.is_none() && music.logo.is_some() {
            logo = music.logo.clone();
        }
        for it in user_music_dao::select_by_music(ctx, id).await? {
            favorite_ids.insert(it.favoriteId);
            if user_music_dao::count_by_favorite_music(ctx, it.favoriteId, best_id).await? > 0 {
                //歌单里已经有保留的歌曲,直接删除
                user_music_dao::delete_by_favorite_music(ctx, it.favoriteId, id).await?;
            } else {
                user_music_dao::update_music_id(ctx, it.favoriteId, id, best_id).await?;
            }
        }
        music_dao::set_deleted(ctx, id, now).await?;
    }
    if logo != best.logo {
        music_dao::update_logo(ctx, best_id, logo).await?;
    }

    //歌单里的歌曲可能变少了,重新统计歌曲数
    for favorite_id in favorite_ids {
        user_favorite_dao::refresh_music_count(ctx, favorite_id).await?;
    }
    Ok(())
}

/// 歌曲删除多少天之后自动彻底删除
const PURGE_DAYS: i64 = 30;

/// 彻底删除已经标记为删除的歌曲:删除数据库记录,以及DairoDFS里的歌曲文件、视频、封面(别的地方还在使用的文件不删除)
pub async fn purge(id: i64) -> Result<(), crate::web::biz_error::BizError> {
    use crate::dao::{music_collect_dao, music_complaint_dao, music_correct_dao, music_dao, music_file_dao, music_lrc_dao, user_favorite_dao, user_music_dao};
    use crate::service::{file_clean_service, video_task};
    use crate::web::controller::api::music_upload;
    use std::collections::BTreeSet;

    if video_task::processing_id() == id {
        return Err(crate::biz_error!("歌曲的视频正在处理中,请稍后再试"));
    }
    let mut ctx = lib_db::get_context();
    let Ok(music) = music_dao::select_one(&mut ctx, id).await else {
        return Err(crate::biz_error!("歌曲不存在"));
    };
    if music.deleteFlag == 0 {
        return Err(crate::biz_error!("请先删除歌曲,再彻底删除"));
    }
    let collect = music_collect_dao::select_by_music(&mut ctx, id).await?;
    if collect.as_ref().is_some_and(|it| it.state == 0) {
        return Err(crate::biz_error!("歌曲正在处理中,请稍后再试"));
    }
    let mut ctx = crate::db::begin().await?;

    //要从DairoDFS删除的文件
    let mut paths: Vec<String> = music_file_dao::select_by_music(&mut ctx, id).await?.into_iter().map(|it| it.path).collect();
    paths.extend(video_task::delete_videos(&mut ctx, id).await?);
    paths.extend(music.logo);

    //收藏了这首歌的歌单
    let favorite_ids: BTreeSet<i64> = user_music_dao::select_by_music(&mut ctx, id).await?.into_iter().map(|it| it.favoriteId).collect();
    user_music_dao::delete_by_music(&mut ctx, id).await?;
    for favorite_id in favorite_ids {
        user_favorite_dao::refresh_music_count(&mut ctx, favorite_id).await?;
    }
    music_file_dao::delete_by_music(&mut ctx, id).await?;
    music_lrc_dao::delete_by_music(&mut ctx, id).await?;
    music_collect_dao::delete_by_music(&mut ctx, id).await?;
    music_correct_dao::delete_by_music(&mut ctx, id).await?;
    music_complaint_dao::delete_by_music(&mut ctx, id).await?;
    music_dao::delete(&mut ctx, id).await?;
    ctx.commit().await?;

    //上传之后还没有处理的歌曲文件、视频
    if let Some(extra) = collect.filter(|it| it.platform.as_deref() == Some(collect_service::PLATFORM_USER_UPLOAD)).and_then(|it| it.extra) {
        let _ = tokio::fs::remove_file(music_upload::upload_dir().join(extra)).await;
    }
    video_task::remove_upload(id).await;
    file_clean_service::remove_unused(paths).await;
    Ok(())
}

/// 启动后台任务:删除超过一段时间的歌曲自动彻底删除
pub fn start_purge_task() {
    use crate::application;
    use std::time::Duration;

    tokio::spawn(async {
        //启动一分钟之后执行第一次,之后每小时执行一次
        let mut wait = Duration::from_secs(60);
        loop {
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = application::SHUTDOWN_NOTIFY.notified() => break,
            }
            purge_expired().await;
            wait = Duration::from_secs(3600);
        }
    });
}

/// 彻底删除超过保留时间的歌曲
async fn purge_expired() {
    use crate::dao::music_dao;
    use crate::util::time_util;

    let before = time_util::current_millis() - PURGE_DAYS * 24 * 3600 * 1000;
    let mut ctx = lib_db::get_context();
    let ids = match music_dao::select_purge_ids(&mut ctx, before, 200).await {
        Ok(it) => it,
        Err(e) => {
            eprintln!("-->获取要彻底删除的歌曲失败: {}", e);
            return;
        }
    };
    for id in ids {
        match purge(id).await {
            Ok(_) => println!("-->歌曲{}删除已超过{}天,已彻底删除", id, PURGE_DAYS),
            Err(e) => eprintln!("-->彻底删除歌曲{}失败: {:?}", id, e),
        }
    }
}
