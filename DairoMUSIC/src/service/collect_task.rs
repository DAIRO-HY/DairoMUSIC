//! 后台处理采集任务(采集平台的歌曲、会员上传的歌曲)
//! 1. 得到源文件:采集平台的歌曲等采集服务采集完成后下载,会员上传的歌曲直接使用上传的文件
//! 2. 用ffprobe读取时长、比特率、歌名等信息
//! 3. 转码成不同音质的mp3(只从高音质转成低音质)
//! 4. 上传到DairoDFS,保存封面和歌词
//! 5. 和曲库里已有的歌曲文件相同时,合并到已有的歌曲

use crate::application;
use crate::dao::music_collect_dao::MusicCollect;
use crate::dao::music_file_dao::MusicFile;
use crate::dao::music_lrc_dao::MusicLrc;
use crate::dao::{music_collect_dao, music_dao, music_file_dao, music_lrc_dao};
use crate::service::{collect_service, dfs_service, music_service};
use crate::util::ffmpeg_util::{self, AudioInfo};
use crate::util::{image_util, time_util};
use crate::web::controller::api::music_upload;
use rand::distr::{Alphanumeric, SampleString};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;

/// 同时处理的任务数
const MAX_PARALLEL: usize = 2;

/// 每个任务最多处理的次数,失败超过这个次数后不再处理
const MAX_COUNT: i64 = 5;

/// 失败的任务过多长时间再重试(毫秒)
const RETRY_INTERVAL: i64 = 10 * 60 * 1000;

/// 转码的目标音质,源文件音质比目标音质高时才转码
const BIT_RATES: [i64; 2] = [320, 128];

/// 等待采集服务采集完成的最长时间
const COLLECT_WAIT: Duration = Duration::from_secs(30 * 60);

/// 唤醒任务
static WAKE: Notify = Notify::const_new();

/// 正在处理的任务ID
static RUNNING: LazyLock<Mutex<HashSet<i64>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// HTTP客户端(下载歌曲、封面、歌词)
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(60))
        .build()
        .unwrap()
});

/// 有新任务时唤醒
pub fn wake() {
    WAKE.notify_one();
}

/// 启动后台任务
pub fn start() {
    tokio::spawn(async {
        //上次没有处理完的任务(程序被关闭)重新处理
        let mut ctx = lib_db::get_context();
        if let Err(e) = music_collect_dao::reset_collecting(&mut ctx).await {
            eprintln!("-->恢复采集任务失败: {}", e);
        }
        loop {
            if let Err(e) = dispatch().await {
                eprintln!("-->获取采集任务失败: {}", e);
            }
            tokio::select! {
                _ = WAKE.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                _ = application::SHUTDOWN_NOTIFY.notified() => break,
            }
        }
    });
}

/// 取出需要处理的任务开始处理
async fn dispatch() -> Result<(), sqlx::Error> {
    let running_count = RUNNING.lock().unwrap().len();
    if running_count >= MAX_PARALLEL {
        return Ok(());
    }

    //没有启用采集时只处理会员上传的歌曲
    let mut platforms = vec![collect_service::PLATFORM_USER_UPLOAD.to_string()];
    if collect_service::is_enabled() {
        platforms.push(collect_service::PLATFORM_YOUTUBE.to_string());
    }
    let now = time_util::current_millis();
    let mut ctx = lib_db::get_context();
    let list = music_collect_dao::select_need(&mut ctx, now - RETRY_INTERVAL, MAX_COUNT, platforms, (MAX_PARALLEL + running_count) as i64).await?;
    for collect in list {
        {
            let mut running = RUNNING.lock().unwrap();
            if running.len() >= MAX_PARALLEL || running.contains(&collect.id) {
                continue;
            }
            running.insert(collect.id);
        }
        if music_collect_dao::set_collecting(&mut ctx, collect.id, now).await? == 0 {
            //已经被处理了
            RUNNING.lock().unwrap().remove(&collect.id);
            continue;
        }
        tokio::spawn(async move {
            let id = collect.id;
            run(collect).await;
            RUNNING.lock().unwrap().remove(&id);

            //处理下一个
            wake();
        });
    }
    Ok(())
}

/// 处理一个任务,记录处理结果
async fn run(collect: MusicCollect) {
    let start = std::time::Instant::now();
    let work_dir = PathBuf::from(&application::ARGS.data_dir).join("temp").join("collect").join(collect.id.to_string());
    let result = process(&collect, &work_dir).await;
    let _ = tokio::fs::remove_dir_all(&work_dir).await;
    let now = time_util::current_millis();
    let mut ctx = lib_db::get_context();
    let is_upload = collect.platform.as_deref() == Some(collect_service::PLATFORM_USER_UPLOAD);
    match result {
        Ok(memo) => {
            let memo = format!("{},耗时{}秒", memo, start.elapsed().as_secs());
            let _ = music_collect_dao::set_result(&mut ctx, collect.id, 1, memo, now).await;
            if is_upload {
                let _ = tokio::fs::remove_file(upload_file(&collect)).await;
            }
        }
        Err(e) => {
            eprintln!("-->采集失败(采集ID:{}): {}", collect.id, e);
            let _ = music_collect_dao::set_result(&mut ctx, collect.id, 2, format!("失败: {}", e), now).await;
            let _ = music_dao::set_collect_fail(&mut ctx, collect.musicId).await;

            //不会再重试时,删除上传的文件
            if is_upload && collect.downloadCount + 1 >= MAX_COUNT {
                let _ = tokio::fs::remove_file(upload_file(&collect)).await;
            }
        }
    }
}

/// 会员上传的文件
fn upload_file(collect: &MusicCollect) -> PathBuf {
    music_upload::upload_dir().join(collect.extra.as_deref().unwrap_or_default())
}

/// 更新处理进度
async fn memo(collect: &MusicCollect, text: &str) {
    let mut ctx = lib_db::get_context();
    let _ = music_collect_dao::set_memo(&mut ctx, collect.id, text.to_string(), time_util::current_millis()).await;
}

/// 转码后的文件
struct AudioFile {
    path: PathBuf,
    format: String,
    bit_rate: i64,
    size: i64,
    md5: String,
}

/// 处理一个任务,成功时返回处理结果说明
async fn process(collect: &MusicCollect, work_dir: &Path) -> Result<String, String> {
    tokio::fs::create_dir_all(work_dir).await.map_err(|e| e.to_string())?;
    let mut ctx = lib_db::get_context();
    let music = music_dao::select_one(&mut ctx, collect.musicId).await.map_err(|_| "歌曲不存在".to_string())?;
    let extra = collect.extra.clone().unwrap_or_default();
    let is_youtube = collect.platform.as_deref() == Some(collect_service::PLATFORM_YOUTUBE);

    //1.得到源文件
    let source = if is_youtube {
        if !collect_service::is_enabled() {
            return Err("没有启用采集".to_string());
        }
        wait_collected(collect, &extra).await?;
        memo(collect, "下载中").await;
        let url = format!("{}/d/music?videoId={}", collect_service::api_url(), extra);
        let _ = music_collect_dao::set_url(&mut ctx, collect.id, url.clone()).await;
        let path = work_dir.join("source");
        download(&url, &path, 1024 * 1024 * 1024).await?;
        path
    } else {
        let path = upload_file(collect);
        if !path.exists() {
            return Err("上传的文件不存在".to_string());
        }
        path
    };

    //2.读取歌曲信息
    memo(collect, "读取歌曲信息").await;
    let info = ffmpeg_util::probe(&source).await?;

    //3.转码
    let mut files = vec![audio_file(source.clone(), info.format.clone(), info.bit_rate).await?];
    for rate in BIT_RATES {
        //源文件不是mp3时,至少转一个mp3,保证所有客户端都能播放
        let need_mp3 = info.format != "mp3" && rate == BIT_RATES[BIT_RATES.len() - 1] && files.iter().all(|it| it.format != "mp3");
        if info.bit_rate <= rate && !need_mp3 {
            continue;
        }
        memo(collect, &format!("转码中:{}k", rate)).await;
        let target = work_dir.join(format!("{}.mp3", rate));
        ffmpeg_util::transcode_mp3(&source, &target, rate).await?;
        files.push(audio_file(target, "mp3".to_string(), rate).await?);
    }

    //4.和曲库里已有的歌曲相同时,合并到已有的歌曲
    for file in &files {
        let existing = music_file_dao::select_by_md5(&mut ctx, file.md5.clone()).await.map_err(|e| e.to_string())?;
        if let Some(existing) = existing.filter(|it| it.musicId != collect.musicId) {
            return merge_to_existing(collect, &files, existing.musicId).await;
        }
    }

    //5.上传
    let mut music_files = Vec::new();
    for file in &files {
        memo(collect, &format!("上传中:{}k", file.bit_rate)).await;
        music_files.push(upload(collect.musicId, file, &info).await?);
    }

    //封面:优先使用文件里的图片,没有时下载采集平台的封面
    memo(collect, "保存封面").await;
    let logo = save_cover(&source, &info, music.logo.as_deref()).await;

    //歌词
    let lrc = if is_youtube { get_lrc(&extra).await } else { None };

    //6.更新数据库
    let max_bit_rate = files.iter().map(|it| it.bit_rate).max().unwrap_or(0);
    let name = if is_youtube {
        //采集平台搜索时的歌名已经转换成简体,文件里的歌名是原始的标题,不使用
        music.name.clone()
    } else {
        info.title.clone().or(music.name.clone())
    };
    let mut ctx = crate::db::begin().await.map_err(|e| e.to_string())?;
    for it in &music_files {
        music_file_dao::insert(&mut ctx, it).await.map_err(|e| e.to_string())?;
    }
    let param = music_dao::UpdateCollectedParam {
        id: collect.musicId,
        name,
        artist: info.artist.clone().or(music.artist.clone()),
        album: info.album.clone().or(music.album.clone()),
        logo: logo.or(music.logo.clone()),
        duration: info.duration,
        year: info.year.or(music.year),
        max_bit_rate,
    };
    music_dao::update_collected(&mut ctx, param).await.map_err(|e| e.to_string())?;
    if let Some(lrc) = lrc {
        let lrc = MusicLrc {
            musicId: collect.musicId,
            title: "YouTube采集".to_string(),
            lrc,
            source: Some(collect_service::PLATFORM_YOUTUBE.to_string()),
            ..Default::default()
        };
        music_lrc_dao::insert(&mut ctx, &lrc).await.map_err(|e| e.to_string())?;
    }
    ctx.commit().await.map_err(|e| e.to_string())?;
    Ok(format!("采集成功,{}个文件", music_files.len()))
}

/// 读取文件大小和MD5
async fn audio_file(path: PathBuf, format: String, bit_rate: i64) -> Result<AudioFile, String> {
    let (size, md5) = file_md5(&path).await?;
    Ok(AudioFile { path, format, bit_rate, size, md5 })
}

/// 计算文件大小和MD5(放到阻塞线程里执行)
pub(crate) async fn file_md5(path: &Path) -> Result<(i64, String), String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let mut file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut context = md5::Context::new();
        let mut buf = vec![0u8; 256 * 1024];
        let mut size = 0i64;
        loop {
            let n = file.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            size += n as i64;
            context.consume(&buf[..n]);
        }
        Ok((size, format!("{:x}", context.finalize())))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 上传一个文件,返回歌曲文件记录
async fn upload(music_id: i64, file: &AudioFile, info: &AudioInfo) -> Result<MusicFile, String> {
    let name = Alphanumeric.sample_string(&mut rand::rng(), 16);
    let month = chrono::Local::now().format("%Y%m");
    let path = format!("/music/{}/{}.{}", month, name, file.format);
    let content_type = if file.format == "mp3" { "audio/mpeg" } else { "" };
    dfs_service::upload_file(&file.path, &file.md5, &path, content_type)
        .await
        .map_err(|e| e.to_string())?;
    Ok(MusicFile {
        id: 0,
        musicId: music_id,
        bitRate: Some(file.bit_rate),
        sampleRate: info.sample_rate,
        dataStartPosition: None,
        sampleCount: None,
        frameLength: None,
        size: file.size,
        format: file.format.clone(),
        md5: Some(file.md5.clone()),
        path,
    })
}

/// 和曲库里已有的歌曲相同时:比已有歌曲音质更好的文件补充给已有的歌曲,当前歌曲合并到已有的歌曲
async fn merge_to_existing(collect: &MusicCollect, files: &[AudioFile], existing_id: i64) -> Result<String, String> {
    let mut ctx = lib_db::get_context();
    let existing_files = music_file_dao::select_by_music(&mut ctx, existing_id).await.map_err(|e| e.to_string())?;
    let existing_max = existing_files.iter().filter_map(|it| it.bitRate).max().unwrap_or(0);
    let existing_md5: HashSet<_> = existing_files.iter().filter_map(|it| it.md5.clone()).collect();
    let mut better = Vec::new();
    for file in files.iter().filter(|it| it.bit_rate > existing_max && !existing_md5.contains(&it.md5)) {
        memo(collect, &format!("上传中:{}k", file.bit_rate)).await;
        let info = AudioInfo::default();
        better.push(upload(existing_id, file, &info).await?);
    }
    let mut ctx = crate::db::begin().await.map_err(|e| e.to_string())?;
    for it in &better {
        music_file_dao::insert(&mut ctx, it).await.map_err(|e| e.to_string())?;
    }
    if let Some(max) = better.iter().filter_map(|it| it.bitRate).max() {
        music_dao::update_max_bit_rate(&mut ctx, existing_id, max).await.map_err(|e| e.to_string())?;
    }
    music_service::merge(&mut ctx, &[collect.musicId], existing_id).await.map_err(|e| e.to_string())?;
    ctx.commit().await.map_err(|e| e.to_string())?;
    Ok(format!("和歌曲{}相同,已合并", existing_id))
}

/// 等待采集服务采集完成
async fn wait_collected(collect: &MusicCollect, video_id: &str) -> Result<(), String> {
    let url = format!("{}/d/collect_info", collect_service::api_url());
    let start = std::time::Instant::now();
    let mut error_count = 0;
    loop {
        if start.elapsed() > COLLECT_WAIT {
            return Err("等待采集服务采集超时".to_string());
        }
        let res = CLIENT.get(&url).query(&[("videoId", video_id)]).timeout(Duration::from_secs(10)).send().await;
        match res {
            Ok(res) if res.status().is_success() => {
                let text = res.text().await.unwrap_or_default();
                if text.trim() == "OK" {
                    return Ok(());
                }
                error_count = 0;
                memo(collect, &format!("采集服务:{}", text.trim())).await;
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

/// 下载文件
/// - max_size 最大允许的大小
pub(crate) async fn download(url: &str, path: &Path, max_size: u64) -> Result<(), String> {
    let mut res = CLIENT.get(url).send().await.map_err(|e| format!("下载失败: {}", e))?;
    if !res.status().is_success() {
        return Err(format!("下载失败: {}", res.status()));
    }
    let mut file = tokio::fs::File::create(path).await.map_err(|e| e.to_string())?;
    let mut size = 0u64;
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("下载中断: {}", e))? {
        size += chunk.len() as u64;
        if size > max_size {
            return Err("文件太大".to_string());
        }
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
    }
    file.flush().await.map_err(|e| e.to_string())?;
    if size == 0 {
        return Err("下载的文件为空".to_string());
    }
    Ok(())
}

/// 保存封面到DairoDFS,返回存储路径;没有封面或者失败时返回None
/// - current_logo 歌曲当前的封面(采集平台的封面是网址)
async fn save_cover(source: &Path, info: &AudioInfo, current_logo: Option<&str>) -> Option<String> {
    let data = if info.has_cover {
        ffmpeg_util::cover(source).await.ok()
    } else {
        None
    };
    let data = match data {
        Some(data) => data,
        None => {
            //下载采集平台的封面
            let url = current_logo.filter(|it| it.starts_with("http://") || it.starts_with("https://"))?;
            let url = collect_service::to_internal_url(url);
            let res = CLIENT.get(url).timeout(Duration::from_secs(20)).send().await.ok()?;
            if !res.status().is_success() {
                return None;
            }
            let data = res.bytes().await.ok()?;
            if data.len() > 10 * 1024 * 1024 {
                return None;
            }
            data.to_vec()
        }
    };
    let ext = image_util::image_ext(&data)?;
    let name = Alphanumeric.sample_string(&mut rand::rng(), 16);
    let path = format!("/image/logo/{}/{}.{}", time_util::current_year(), name, ext);
    match dfs_service::upload(data, &path, "").await {
        Ok(_) => Some(path),
        Err(e) => {
            eprintln!("-->封面上传失败: {}", e);
            None
        }
    }
}

/// 从采集服务获取歌词,没有或失败时返回None
async fn get_lrc(video_id: &str) -> Option<String> {
    let url = format!("{}/d/lrc", collect_service::api_url());
    let res = CLIENT
        .get(url)
        .query(&[("videoId", video_id)])
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .ok()?;
    if !res.status().is_success() {
        return None;
    }
    let text = res.text().await.ok()?;
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.to_string())
}
