//! 客户端:歌曲

use crate::dao::music_collect_dao::MusicCollect;
use crate::dao::music_dao::Music;
use crate::dao::music_lrc_dao::MusicLrc;
use crate::dao::music_complaint_dao::MusicComplaint;
use crate::dao::music_correct_dao::MusicCorrect;
use crate::dao::{music_video_dao, music_collect_dao, music_complaint_dao, music_correct_dao, music_dao, music_file_dao, music_lrc_dao};
use crate::service::music_service::{self, MusicVo};
use crate::service::{collect_service, dfs_service, video_task};
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::filter::user_auth::LoginUser;
use crate::web::form;
use crate::web::model::non_empty;
use crate::{biz_error, db};
use hyperoute_core::Ext;
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use validator::Validate;

/// 歌曲来源:采集平台
const SOURCE_COLLECT: i64 = 2;

/// 搜索结果的最大数量
const SEARCH_LIMIT: usize = 30;


/// 从曲库搜索歌曲
/// 关键词和按空格、横线拆开的词,歌名或歌手包含任意一个就算匹配,按匹配程度排序
#[GET("/api/music/search")]
pub async fn search(login: Ext<LoginUser>, key: String) -> Result<Vec<MusicVo>, BizError> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Ok(Vec::new());
    }
    if key.chars().count() > 64 {
        return Err(biz_error!("搜索关键词太长"));
    }
    let mut keys = vec![key.clone()];
    for it in key.split([' ', '-']).map(str::trim).filter(|it| !it.is_empty()) {
        if !keys.iter().any(|k| k == it) {
            keys.push(it.to_string());
        }
    }
    let mut ctx = lib_db::get_context();
    let list = music_dao::search(&mut ctx, keys.clone(), login.is_test as i64).await?;
    let upper_keys: Vec<String> = keys.iter().map(|it| it.to_uppercase()).collect();
    let mut list: Vec<(i64, Music)> = list.into_iter().map(|it| (search_weight(&key, &upper_keys, &it), it)).collect();
    list.sort_by(|a, b| b.0.cmp(&a.0));
    list.truncate(SEARCH_LIMIT);

    //带上播放用的文件
    let ids: Vec<i64> = list.iter().map(|(_, it)| it.id).collect();
    let files = if ids.is_empty() {
        Vec::new()
    } else {
        music_file_dao::select_by_musics(&mut ctx, ids).await?
    };
    let file_map = music_service::group_files(&files);
    let result = list
        .into_iter()
        .map(|(_, it)| {
            MusicVo {
                id: it.id,
                name: it.name,
                artist: it.artist,
                album: it.album,
                logo: dfs_service::file_url(it.logo.as_deref()),
                duration: it.duration,
                video_state: it.videoState,
                files: music_service::files_of(&file_map, it.id),
                ..Default::default()
            }
        })
        .collect();
    Ok(result)
}

/// 搜索结果的排序权重,和老版本的规则一致
fn search_weight(key: &str, upper_keys: &[String], music: &Music) -> i64 {
    let name = music.name.clone().unwrap_or_default();
    let upper_name = name.to_uppercase();
    let upper_key = key.to_uppercase();
    let mut weight = 0;
    if name == key {
        weight += 100_000_000;
    }
    if upper_name == upper_key {
        weight += 10_000_000;
    }
    if name.contains(key) {
        weight += 1_000_000;
    }
    if upper_name.contains(&upper_key) {
        weight += 100_000;
    }
    if upper_keys.contains(&upper_name) {
        weight += 1_000;
    }
    if upper_keys.iter().any(|it| upper_name.contains(it.as_str())) {
        weight += 100;
    }
    for value in [&music.artist, &music.album, &music.tag] {
        let Some(value) = value.as_ref().filter(|it| !it.is_empty()) else {
            continue;
        };
        let upper = value.to_uppercase();
        if upper_keys.contains(&upper) {
            weight += 10;
        }
        if upper_keys.iter().any(|it| upper.contains(it.as_str())) {
            weight += 1;
        }
    }
    weight
}

/// 从采集平台搜索
#[GET("/api/music/search_collect")]
pub async fn search_collect(login: Ext<LoginUser>, key: String) -> Result<Vec<MusicVo>, BizError> {
    let key = key.trim().replace(' ', "");
    if login.is_test || key.is_empty() {
        //测试账号不能从采集平台搜索
        return Ok(Vec::new());
    }
    if key.chars().count() > 64 {
        return Err(biz_error!("搜索关键词太长"));
    }
    let list = match collect_service::search(&key).await {
        Ok(list) => list,
        Err(e) => {
            eprintln!("-->采集平台搜索失败: {}", e);
            return Err(biz_error!("采集平台暂时不可用,请稍后再试"));
        }
    };
    let result = list
        .into_iter()
        .map(|it| MusicVo {
            name: Some(it.name),
            logo: it.logo,
            collect: music_service::collect_vo(Some(it.platform), Some(it.extra)),
            ..Default::default()
        })
        .collect();
    Ok(result)
}

/// 歌曲详情
#[GET("/api/music/detail")]
pub async fn detail(id: i64) -> Result<MusicVo, BizError> {
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
        favorite_id: None,
        files: music_service::file_vos(&files),
        collect: music_service::collect_vo(music.platform, music.extra),
    })
}

/// 歌词
#[GET("/api/music/lrc")]
pub async fn lrc(id: i64) -> Result<Vec<MusicLrc>, BizError> {
    let mut ctx = lib_db::get_context();
    Ok(music_lrc_dao::select_by_music(&mut ctx, id).await?)
}

/// 添加采集:采集平台的搜索结果加入曲库,返回歌曲ID(已经添加过时直接返回)
#[POST("/api/music/collect")]
pub async fn collect(login: Ext<LoginUser>, form: form::MusicCollect) -> Result<i64, BizError> {
    if login.is_test {
        return Err(BizError::forbidden());
    }
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    if form.platform != collect_service::PLATFORM_YOUTUBE || !collect_service::is_enabled() {
        return Err(BizError::field("platform", "采集平台不正确"));
    }

    //YouTube的视频ID只有字母、数字、下划线和横线
    if !form.extra.chars().all(|it| it.is_ascii_alphanumeric() || it == '_' || it == '-') {
        return Err(BizError::field("extra", "采集参数不正确"));
    }
    let mut ctx = db::begin().await?;
    if let Some(it) = music_collect_dao::select_by_extra(&mut ctx, form.extra.clone()).await? {
        return Ok(it.musicId);
    }
    let now = time_util::current_millis();
    let music = Music {
        name: Some(form.name),
        logo: non_empty(Some(form.logo)),
        sourceType: SOURCE_COLLECT,
        date: now,
        ..Default::default()
    };
    let music_id = music_dao::insert(&mut ctx, &music).await?;
    let collect = MusicCollect {
        musicId: music_id,
        extra: Some(form.extra),
        platform: Some(form.platform),
        date: now,
        state: -1,
        updateTime: now,
        ..Default::default()
    };
    music_collect_dao::insert(&mut ctx, &collect).await?;
    ctx.commit().await?;
    crate::service::collect_task::wake();

    //同时采集视频(采集服务可能还缓存着用户刚刚预览过的视频)
    video_task::request(music_id).await?;
    Ok(music_id)
}

/// 歌曲视频
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoVo {
    // 视频状态 0:未采集 1:采集中 2:有视频 3:没有视频(只有一张图片) 4:后台删除了视频 5:采集失败
    pub state: i64,

    // 原画
    pub original: Option<VideoFileVo>,

    // 流畅(原画不高于480p 25fps时和原画是同一个文件)
    pub smooth: Option<VideoFileVo>,
}

/// 视频文件
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VideoFileVo {
    // 文件ID(客户端可以用来做缓存的key)
    pub id: i64,

    // 清晰度 1:原画 2:流畅
    pub quality: i64,

    // 状态 0:转码中 1:可以播放 2:转码失败
    pub state: i64,
    pub width: i64,
    pub height: i64,
    pub codec: Option<String>,

    // 比特率(kbps)
    pub bit_rate: Option<i64>,
    pub frame_rate: Option<i64>,

    // 时长(毫秒)
    pub duration: Option<i64>,
    pub size: Option<i64>,
    pub md5: Option<String>,

    // 播放地址,可以播放时才有
    pub url: Option<String>,
}

/// 获取歌曲视频;还没有采集过视频(老的歌曲)或者采集失败超过一段时间时,开始采集
/// 采集、转码过程中客户端每隔几秒请求一次,直到可以播放
#[GET("/api/music/video")]
pub async fn video(id: i64) -> Result<VideoVo, BizError> {
    let mut ctx = lib_db::get_context();
    let Ok(music) = music_dao::select_one(&mut ctx, id).await else {
        return Err(BizError::not_found());
    };
    if music.deleteFlag != 0 {
        return Err(BizError::not_found());
    }
    let mut state = music.videoState;
    if (state == video_task::VIDEO_NONE || state == video_task::VIDEO_FAIL) && video_task::request(id).await? {
        state = video_task::VIDEO_COLLECTING;
    }
    let mut result = VideoVo { state, original: None, smooth: None };
    if state != video_task::VIDEO_OK {
        return Ok(result);
    }
    for it in music_video_dao::select_by_music(&mut ctx, id).await? {
        let vo = VideoFileVo {
            id: it.id,
            quality: it.quality,
            state: it.state,
            width: it.width,
            height: it.height,
            codec: it.codec,
            bit_rate: it.bitRate,
            frame_rate: it.frameRate,
            duration: it.duration,
            size: it.size,
            md5: it.md5,
            url: if it.state == 1 { dfs_service::file_url(it.path.as_deref()) } else { None },
        };
        if it.quality == video_task::QUALITY_ORIGINAL {
            result.original = Some(vo);
        } else {
            result.smooth = Some(vo);
        }
    }

    //原画不高于480p 25fps时没有单独的流畅版,使用原画
    if result.smooth.is_none() {
        result.smooth = result.original.clone();
    }
    Ok(result)
}

/// 纠正歌名、歌手、专辑;信赖用户提交的直接生效,其他用户的等后台处理
#[POST("/api/music/correct")]
pub async fn correct(login: Ext<LoginUser>, form: form::MusicCorrect) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let mut ctx = db::begin().await?;
    let Ok(music) = music_dao::select_one(&mut ctx, form.id).await else {
        return Err(BizError::not_found());
    };
    let mut changes = Vec::new();
    let mut name = music.name.clone();
    let mut artist = music.artist.clone();
    let mut album = music.album.clone();
    for (field, value, current) in [("name", form.name, &mut name), ("artist", form.artist, &mut artist), ("album", form.album, &mut album)] {
        let value = value.trim().to_string();
        if value.is_empty() || Some(&value) == current.as_ref() {
            continue;
        }
        changes.push(CorrectData {
            field,
            before: current.clone(),
            after: value.clone(),
        });
        *current = Some(value);
    }
    if changes.is_empty() {
        return Ok(());
    }
    let correct = MusicCorrect {
        musicId: form.id,
        data: serde_json::to_string(&changes).unwrap(),
        state: if login.is_friend { 1 } else { 0 },
        userId: login.id,
        date: time_util::current_millis(),
        ..Default::default()
    };
    music_correct_dao::insert(&mut ctx, &correct).await?;
    if login.is_friend {
        music_dao::correct(&mut ctx, form.id, name, artist, album).await?;
    }
    ctx.commit().await?;
    Ok(())
}

/// 纠错内容
#[derive(Serialize)]
struct CorrectData {
    field: &'static str,
    before: Option<String>,
    after: String,
}

/// 投诉歌曲
#[POST("/api/music/complaint")]
pub async fn complaint(login: Ext<LoginUser>, form: form::Content) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let complaint = MusicComplaint {
        userId: login.id,
        musicId: form.id,
        content: form.content,
        date: time_util::current_millis(),
        ..Default::default()
    };
    let mut ctx = lib_db::get_context();
    music_complaint_dao::insert(&mut ctx, &complaint).await?;
    Ok(())
}

/// 投诉理由的选项
#[GET("/api/music/complaint_samples")]
pub async fn complaint_samples() -> Vec<&'static str> {
    vec!["音质不符", "与歌名不符", "歌曲不完整", "被植入广告", "没有声音", "歌曲不合法"]
}
