//! 后台处理歌曲视频:从采集平台采集(只有采集平台的歌曲),或者后台手动上传(所有歌曲都可以)
//! 视频文件大、转码很耗CPU,同一时间只处理一首歌曲;手动上传的优先处理
//! 1. 等采集服务采集好最高规格的视频后下载(手动上传的直接使用上传的文件)
//! 2. 判断是不是只有一张图片,只有图片时认为没有视频(手动上传的不判断)
//! 3. 原画高于480p或高于25fps时先转码流畅版(480p 25fps)(很快就能播放),再处理原画(VP9、AV1等转码成HEVC)
//! 4. 上传到DairoDFS

use crate::application;
use crate::dao::music_video_dao::{self, MusicVideo};
use crate::dao::music_dao;
use crate::service::{collect_service, collect_task, dfs_service, file_clean_service};
use crate::util::time_util;
use crate::util::video_util::{self, VideoInfo};
use rand::distr::{Alphanumeric, SampleString};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use tokio::sync::Notify;

/// 视频状态 0:未采集 1:采集中 2:有视频 3:没有视频(只有一张图片) 4:后台删除了视频 5:采集失败
pub const VIDEO_NONE: i64 = 0;
pub const VIDEO_COLLECTING: i64 = 1;
pub const VIDEO_OK: i64 = 2;
pub const VIDEO_STATIC: i64 = 3;
pub const VIDEO_DELETED: i64 = 4;
pub const VIDEO_FAIL: i64 = 5;

/// 清晰度 1:原画 2:流畅
pub const QUALITY_ORIGINAL: i64 = 1;
pub const QUALITY_SMOOTH: i64 = 2;

/// 视频文件状态 0:转码中 1:可以播放 2:转码失败
const FILE_PROCESSING: i64 = 0;

/// 采集失败的视频过多长时间才允许重试(毫秒)
pub const RETRY_INTERVAL: i64 = 60 * 60 * 1000;

/// 等待采集服务采集视频的最长时间
const COLLECT_WAIT: Duration = Duration::from_secs(2 * 60 * 60);

/// 采集服务连续返回几次"采集失败"后放弃(采集服务每60秒重试一次)
const MAX_FAIL_REPLY: usize = 3;

/// 允许的最大视频文件
const MAX_SIZE: u64 = 20 * 1024 * 1024 * 1024;

/// 后台可以上传的视频格式
pub const UPLOAD_EXTS: [&str; 8] = ["mp4", "mov", "m4v", "mkv", "webm", "avi", "flv", "ts"];

/// 后台上传的视频允许的最大大小
pub const UPLOAD_MAX_SIZE: u64 = MAX_SIZE;

/// 视频的来源
enum Source {
    // 采集平台,参数是采集平台的视频ID
    Platform(String),

    // 后台手动上传,参数是上传的文件
    Upload(PathBuf),
}

/// 后台手动上传的视频的保存目录,文件名是 {歌曲ID}.{扩展名},上传完成之后由后台任务处理
pub fn upload_dir() -> PathBuf {
    PathBuf::from(&application::ARGS.data_dir).join("upload_video")
}

/// 上传目录里等待处理的视频:(歌曲ID, 文件),还在上传的文件(.uploading)不算
async fn pending_uploads() -> Vec<(i64, PathBuf)> {
    let mut list = Vec::new();
    let Ok(mut dir) = tokio::fs::read_dir(upload_dir()).await else {
        return list;
    };
    while let Ok(Some(entry)) = dir.next_entry().await {
        let path = entry.path();
        let ext = path.extension().and_then(|it| it.to_str()).unwrap_or_default().to_lowercase();
        let id = path.file_stem().and_then(|it| it.to_str()).and_then(|it| it.parse::<i64>().ok());
        if let Some(id) = id.filter(|_| UPLOAD_EXTS.contains(&ext.as_str())) {
            list.push((id, path));
        }
    }
    list.sort();
    list
}

/// 删除歌曲等待处理的上传视频(重新上传、删除视频、重新采集时)
pub async fn remove_upload(music_id: i64) {
    for (id, path) in pending_uploads().await {
        if id == music_id {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
}

/// 删除歌曲的视频记录,返回视频文件的存储路径(数据库提交之后用file_clean_service从DairoDFS删除)
pub async fn delete_videos(ctx: &mut sqlx_context::DbContext, music_id: i64) -> Result<Vec<String>, sqlx::Error> {
    let videos = music_video_dao::select_by_music(ctx, music_id).await?;
    music_video_dao::delete_by_music(ctx, music_id).await?;
    Ok(videos.into_iter().filter_map(|it| it.path).collect())
}

/// 唤醒任务
static WAKE: Notify = Notify::const_new();

/// 正在处理的歌曲ID,没有时为0
static PROCESSING: AtomicI64 = AtomicI64::new(0);

/// 正在处理的歌曲ID,没有时为0
pub fn processing_id() -> i64 {
    PROCESSING.load(Ordering::Relaxed)
}

/// HTTP客户端(查询采集进度)
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap()
});

/// 有新任务时唤醒
pub fn wake() {
    WAKE.notify_one();
}

/// 请求采集歌曲的视频(还没有采集过,或者失败超过一段时间时),返回是否开始采集
pub async fn request(music_id: i64) -> Result<bool, sqlx::Error> {
    if !collect_service::is_video_enabled() {
        return Ok(false);
    }
    let now = time_util::current_millis();
    let mut ctx = lib_db::get_context();
    let count = music_dao::request_video(&mut ctx, music_id, now, now - RETRY_INTERVAL).await?;
    if count > 0 {
        wake();
    }
    Ok(count > 0)
}

/// 启动后台任务
pub fn start() {
    tokio::spawn(async {
        //上次没有处理完的视频(程序被关闭)重新采集
        if let Err(e) = reset_processing().await {
            eprintln!("-->恢复视频任务失败: {}", e);
        }
        loop {
            //一首处理完后马上处理下一首,没有任务时等待唤醒
            let has_next = match next().await {
                Ok(it) => it,
                Err(e) => {
                    eprintln!("-->获取视频任务失败: {}", e);
                    false
                }
            };
            if has_next {
                continue;
            }
            tokio::select! {
                _ = WAKE.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                _ = application::SHUTDOWN_NOTIFY.notified() => break,
            }
        }
    });
}

/// 没有处理完的视频删除后重新采集
async fn reset_processing() -> Result<(), sqlx::Error> {
    let mut ctx = lib_db::get_context();
    let now = time_util::current_millis();
    for music_id in music_video_dao::select_processing_music(&mut ctx).await? {
        let paths = delete_videos(&mut ctx, music_id).await?;
        music_dao::set_video_state(&mut ctx, music_id, VIDEO_COLLECTING, now).await?;
        file_clean_service::remove_unused(paths).await;
    }
    Ok(())
}

/// 处理下一首歌曲,返回是否处理了
async fn next() -> Result<bool, sqlx::Error> {
    //后台手动上传的视频优先处理,没有启用视频采集时也处理
    if let Some((music_id, file)) = pending_uploads().await.into_iter().next() {
        if current_state(music_id).await.is_some() {
            println!("-->开始处理上传的视频:歌曲{}", music_id);
            run(music_id, Source::Upload(file.clone())).await?;
        }

        //处理完成、失败,或者歌曲已经不存在时都删除上传的文件(程序中途被关闭时文件还在,下次启动重新处理)
        if let Err(e) = tokio::fs::remove_file(&file).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("-->删除上传的视频失败:{} {}", file.display(), e);

            //删除不掉时不能马上再处理,否则会一直重复转码
            return Ok(false);
        }
        return Ok(true);
    }
    if !collect_service::is_video_enabled() {
        return Ok(false);
    }
    let mut ctx = lib_db::get_context();
    let now = time_util::current_millis();

    //不是采集平台的歌曲(会员上传等)没有视频
    music_dao::set_video_none_not_platform(&mut ctx, collect_service::PLATFORM_YOUTUBE, now).await?;
    let Some(next) = music_dao::select_video_next(&mut ctx, collect_service::PLATFORM_YOUTUBE).await? else {
        return Ok(false);
    };
    let video_id = next.extra.unwrap_or_default();
    println!("-->开始采集视频:歌曲{} {}", next.id, video_id);
    run(next.id, Source::Platform(video_id)).await?;
    Ok(true)
}

/// 处理一首歌曲的视频,并保存处理结果
async fn run(music_id: i64, source: Source) -> Result<(), sqlx::Error> {
    let work_dir = PathBuf::from(&application::ARGS.data_dir).join("temp").join("video").join(music_id.to_string());
    let start = std::time::Instant::now();
    PROCESSING.store(music_id, Ordering::Relaxed);
    let result = process(music_id, &source, &work_dir).await;
    PROCESSING.store(0, Ordering::Relaxed);
    let _ = tokio::fs::remove_dir_all(&work_dir).await;
    let mut ctx = lib_db::get_context();
    let now = time_util::current_millis();
    match result {
        Ok(state) => {
            println!("-->视频处理完成:歌曲{} 状态{} 耗时{}秒", music_id, state, start.elapsed().as_secs());
            music_dao::set_video_state(&mut ctx, music_id, state, now).await?;
        }
        Err(e) => {
            eprintln!("-->视频处理失败:歌曲{} {}", music_id, e);

            //处理过程中被后台删除了视频时,不改状态
            if current_state(music_id).await != Some(VIDEO_DELETED) {
                let paths = delete_videos(&mut ctx, music_id).await?;
                music_dao::set_video_state(&mut ctx, music_id, VIDEO_FAIL, now).await?;
                file_clean_service::remove_unused(paths).await;
            }
        }
    }
    Ok(())
}

/// 歌曲当前的视频状态,歌曲不存在或被删除时为None
async fn current_state(music_id: i64) -> Option<i64> {
    let mut ctx = lib_db::get_context();
    let music = music_dao::select_one(&mut ctx, music_id).await.ok()?;
    (music.deleteFlag == 0).then_some(music.videoState)
}

/// 处理一首歌曲的视频,返回处理后的视频状态
async fn process(music_id: i64, source: &Source, work_dir: &Path) -> Result<i64, String> {
    tokio::fs::create_dir_all(work_dir).await.map_err(|e| e.to_string())?;
    let (source, is_upload) = match source {
        Source::Platform(video_id) => {
            if video_id.is_empty() || !video_id.chars().all(|it| it.is_ascii_alphanumeric() || it == '_' || it == '-') {
                return Err("采集参数不正确".to_string());
            }

            //1.等采集服务采集完成后下载
            wait_collected(video_id).await?;
            let source = work_dir.join("source.mp4");
            let url = format!("{}/d/video?videoId={}&type=raw", collect_service::api_url(), video_id);
            collect_task::download(&url, &source, MAX_SIZE).await?;
            (source, false)
        }

        //后台手动上传的直接使用上传的文件
        Source::Upload(file) => (file.clone(), true),
    };

    //2.读取视频信息,判断是不是只有一张图片
    let info = video_util::probe(&source).await?;
    println!(
        "-->视频信息:歌曲{} {} {}x{} {}fps {}kbps 音频:{}",
        music_id,
        info.codec,
        info.width,
        info.height,
        info.frame_rate,
        info.bit_rate,
        info.audio_codec.as_deref().unwrap_or("无")
    );

    //手动上传的视频是管理员确认过的,不判断是不是只有一张图片
    if !is_upload && video_util::is_static(&source).await? {
        return Ok(VIDEO_STATIC);
    }

    //3.先创建视频记录,客户端可以看到"转码中"
    let need_smooth = video_util::need_smooth(&info);
    let mut ctx = crate::db::begin().await.map_err(|e| e.to_string())?;
    let old_paths = delete_videos(&mut ctx, music_id).await.map_err(|e| e.to_string())?;
    let mut qualities = vec![QUALITY_ORIGINAL];
    if need_smooth {
        qualities.push(QUALITY_SMOOTH);
    }
    let mut ids = Vec::new();
    for quality in &qualities {
        let video = MusicVideo {
            musicId: music_id,
            quality: *quality,
            state: FILE_PROCESSING,
            createTime: time_util::current_millis(),
            ..Default::default()
        };
        ids.push((*quality, music_video_dao::insert(&mut ctx, &video).await.map_err(|e| e.to_string())?));
    }
    music_dao::set_video_state(&mut ctx, music_id, VIDEO_OK, time_util::current_millis())
        .await
        .map_err(|e| e.to_string())?;
    ctx.commit().await.map_err(|e| e.to_string())?;

    //原来的视频文件从DairoDFS删除
    file_clean_service::remove_unused(old_paths).await;

    //4.流畅版先处理(转码快),再处理原画
    let mut success = 0;
    for (quality, id) in ids.into_iter().rev() {
        if current_state(music_id).await != Some(VIDEO_OK) {
            return Err("视频已被删除".to_string());
        }
        let target = work_dir.join(format!("{}.mp4", quality));
        let result = if quality == QUALITY_SMOOTH {
            video_util::transcode_smooth(&source, &target, &info).await
        } else {
            video_util::transcode_original(&source, &target, &info).await
        };
        let result = match result {
            Ok(_) => save(id, music_id, &target, &info).await,
            Err(e) => Err(e),
        };
        let _ = tokio::fs::remove_file(&target).await;
        let mut ctx = lib_db::get_context();
        match result {
            Ok(_) => success += 1,
            Err(e) => {
                eprintln!("-->视频转码失败:歌曲{} 清晰度{} {}", music_id, quality, e);
                let memo: String = e.chars().take(500).collect();
                let _ = music_video_dao::set_fail(&mut ctx, id, memo).await;
            }
        }
    }
    if success == 0 {
        return Err("转码失败".to_string());
    }
    Ok(VIDEO_OK)
}

/// 上传转码后的文件,更新视频记录
async fn save(id: i64, music_id: i64, file: &Path, source_info: &VideoInfo) -> Result<(), String> {
    let info = video_util::probe(file).await?;
    let (size, md5) = collect_task::file_md5(file).await?;
    let name = Alphanumeric.sample_string(&mut rand::rng(), 16);
    let month = chrono::Local::now().format("%Y%m");
    let path = format!("/video/{}/{}.mp4", month, name);

    //上传前再确认一次,转码的时候可能被后台删除了
    if current_state(music_id).await != Some(VIDEO_OK) {
        return Err("视频已被删除".to_string());
    }
    dfs_service::upload_file(file, &md5, &path, "video/mp4").await.map_err(|e| e.to_string())?;
    let memo = format!(
        "源文件:{} {}x{} {}kbps",
        source_info.codec, source_info.width, source_info.height, source_info.bit_rate
    );
    let param = music_video_dao::SetDoneParam {
        id,
        codec: info.codec.clone(),
        width: info.width,
        height: info.height,
        bit_rate: info.bit_rate,
        frame_rate: info.frame_rate,
        duration: info.duration,
        size,
        md5,
        path,
        memo,
    };
    let mut ctx = lib_db::get_context();
    music_video_dao::set_done(&mut ctx, param).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// 等待采集服务采集好视频
async fn wait_collected(video_id: &str) -> Result<(), String> {
    let url = format!("{}/d/video_info", collect_service::api_url());
    let start = std::time::Instant::now();
    let mut error_count = 0;
    let mut fail_count = 0;
    let mut in_fail = false;
    let mut last_seconds = 0;
    loop {
        if start.elapsed() > COLLECT_WAIT {
            return Err("等待采集服务采集视频超时".to_string());
        }
        let res = CLIENT.get(&url).query(&[("videoId", video_id), ("type", "raw")]).send().await;
        match res {
            Ok(res) if res.status().is_success() => {
                error_count = 0;
                let text = res.text().await.unwrap_or_default();
                let text = text.trim();
                if text == "OK" {
                    return Ok(());
                }
                if text.starts_with("采集失败") {
                    //冷却期间每次查询都返回同一次失败,剩余秒数变大(重试后又失败了)才算新的一次失败
                    let seconds = retry_seconds(text);
                    if !in_fail || seconds > last_seconds {
                        fail_count += 1;
                    }
                    in_fail = true;
                    last_seconds = seconds;
                    if fail_count >= MAX_FAIL_REPLY {
                        return Err(format!("采集服务: {}", text));
                    }
                } else {
                    in_fail = false;
                }
            }
            Ok(res) => {
                error_count += 1;
                if error_count > 10 {
                    return Err(format!("采集服务返回错误: {}", res.status()));
                }
            }
            Err(e) => {
                error_count += 1;
                if error_count > 10 {
                    return Err(format!("连接采集服务失败: {}", e));
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(6)).await;
    }
}

/// 从"采集失败(49秒后重试):..."里取出剩余的秒数
fn retry_seconds(text: &str) -> i64 {
    text.split_once('(')
        .and_then(|(_, it)| it.split_once("秒"))
        .and_then(|(it, _)| it.parse().ok())
        .unwrap_or(0)
}
