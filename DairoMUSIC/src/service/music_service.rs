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

    // 播放地址
    pub url: Option<String>,
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
pub fn file_vos(files: &[&MusicFile]) -> Vec<FileVo> {
    let mut files = files.to_vec();
    files.sort_by_key(|it| (it.bitRate.unwrap_or(0), it.id));
    files.into_iter().map(Into::into).collect()
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
pub fn files_of(file_map: &HashMap<i64, Vec<&MusicFile>>, music_id: i64) -> Vec<FileVo> {
    file_map.get(&music_id).map(|files| file_vos(files)).unwrap_or_default()
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
