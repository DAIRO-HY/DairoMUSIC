//! 视频处理:读取视频信息、判断是不是只有一张图片、转码
//! 原画:iPhone能直接播放的编码(H.264、HEVC)只重新封装,其他编码(VP9、AV1等)转码成同分辨率的HEVC
//! 流畅:短边不超过480、帧率不超过25,H.264编码
//! 输出的文件都是mp4,并且把moov放到文件开头(faststart),客户端可以边下边播

use crate::service::config_service;
use crate::util::ffmpeg_util::{ffmpeg, ffprobe, run};
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

/// 流畅版的短边长度
pub const SMOOTH_SIZE: i64 = 480;

/// 流畅版的帧率
pub const SMOOTH_FPS: i64 = 25;

/// 读取视频信息的超时时间
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// 判断是不是只有一张图片的超时时间
const STATIC_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// 转码的超时时间(4K的视频用软件编码可能需要很长时间)
const TRANSCODE_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);

/// 判断图片变化时,把每一帧缩小成这个大小的灰度图
const THUMB_W: usize = 64;
const THUMB_H: usize = 36;

/// 视频信息
#[derive(Debug, Default, Clone)]
pub struct VideoInfo {
    // 视频编码 h264、hevc、vp9、av1等
    pub codec: String,
    pub width: i64,
    pub height: i64,

    // 帧率(取整)
    pub frame_rate: i64,

    // 时长(毫秒)
    pub duration: i64,

    // 整个文件的比特率(kbps)
    pub bit_rate: i64,

    // 像素格式 yuv420p:8位 yuv420p10le:10位
    pub pix_fmt: String,

    // 音频编码,没有音频时为空
    pub audio_codec: Option<String>,
}

impl VideoInfo {
    /// 短边长度(竖屏的视频按宽算)
    pub fn short_side(&self) -> i64 {
        self.width.min(self.height)
    }

    /// iPhone能不能直接播放(不需要转码)
    pub fn is_compatible(&self) -> bool {
        matches!(self.codec.as_str(), "h264" | "hevc")
            && self.audio_codec.as_deref().is_none_or(|it| matches!(it, "aac" | "mp3" | "alac"))
    }
}

/// 读取视频信息
pub async fn probe(path: &Path) -> Result<VideoInfo, String> {
    let path = path.to_string_lossy();
    let out = run(
        &ffprobe(),
        &["-v", "error", "-print_format", "json", "-show_format", "-show_streams", &path],
        PROBE_TIMEOUT,
    )
    .await?;
    let json: Value = serde_json::from_slice(&out).map_err(|e| format!("解析视频信息失败: {}", e))?;
    let streams = json["streams"].as_array().cloned().unwrap_or_default();

    //封面图片也是video类型的流,要排除
    let Some(video) = streams
        .iter()
        .find(|it| it["codec_type"] == "video" && it["disposition"]["attached_pic"] != 1)
    else {
        return Err("文件里没有视频".to_string());
    };
    let audio = streams.iter().find(|it| it["codec_type"] == "audio");
    let number = |value: &Value| -> Option<f64> {
        value.as_str().and_then(|it| it.parse::<f64>().ok()).or_else(|| value.as_f64())
    };
    let format = &json["format"];
    let duration = number(&format["duration"]).or_else(|| number(&video["duration"])).unwrap_or(0.0);
    if duration <= 0.0 {
        return Err("无法获取视频时长".to_string());
    }

    //帧率的格式是 30000/1001 这样的分数
    let frame_rate = video["avg_frame_rate"]
        .as_str()
        .and_then(|it| {
            let (a, b) = it.split_once('/')?;
            let (a, b) = (a.parse::<f64>().ok()?, b.parse::<f64>().ok()?);
            (b > 0.0).then(|| a / b)
        })
        .unwrap_or(0.0);
    Ok(VideoInfo {
        codec: video["codec_name"].as_str().unwrap_or_default().to_string(),
        width: video["width"].as_i64().unwrap_or(0),
        height: video["height"].as_i64().unwrap_or(0),
        frame_rate: frame_rate.round() as i64,
        duration: (duration * 1000.0) as i64,
        bit_rate: (number(&format["bit_rate"]).unwrap_or(0.0) / 1000.0).round() as i64,
        pix_fmt: video["pix_fmt"].as_str().unwrap_or_default().to_string(),
        audio_codec: audio.and_then(|it| it["codec_name"].as_str()).map(|it| it.to_string()),
    })
}

/// 判断视频是不是只有一张图片(YouTube上很多歌曲的视频只是一张封面)
/// 每2秒取一帧,缩小成灰度图后比较相邻两帧的差别,有变化的帧不到10%时认为只有一张图片
pub async fn is_static(path: &Path) -> Result<bool, String> {
    let frames = thumb_frames(path).await?;

    //只解码关键帧时,关键帧间隔大于2秒会重复输出同一帧,去掉完全相同的帧
    let mut unique: Vec<&Vec<u8>> = Vec::new();
    for frame in &frames {
        if unique.last().is_none_or(|last| *last != frame) {
            unique.push(frame);
        }
    }
    if unique.len() < 3 {
        return Ok(true);
    }
    let changed = unique
        .windows(2)
        .filter(|it| {
            let sum: u64 = it[0].iter().zip(it[1].iter()).map(|(a, b)| a.abs_diff(*b) as u64).sum();

            //平均每个像素的差别大于1(满值255)认为画面有变化,只有图片的视频压缩误差一般小于0.5
            sum as f64 / it[0].len() as f64 > 1.0
        })
        .count();
    Ok(changed * 10 < unique.len() - 1)
}

/// 每2秒取一帧,缩小成灰度图
/// 只解码关键帧(H.264、HEVC、AV1等支持,很快),不支持的解码器(如VP9)会解码所有帧
async fn thumb_frames(path: &Path) -> Result<Vec<Vec<u8>>, String> {
    let path = path.to_string_lossy();
    let filter = format!("fps=1/2,scale={}:{},format=gray", THUMB_W, THUMB_H);
    let args = ["-v", "error", "-skip_frame", "nokey", "-i", &path, "-an", "-sn", "-vf", &filter, "-f", "rawvideo", "-"];
    let data = run(&ffmpeg(), &args, STATIC_TIMEOUT).await?;
    Ok(data.chunks_exact(THUMB_W * THUMB_H).map(|it| it.to_vec()).collect())
}

/// 使用的编码器
#[derive(Debug, Clone)]
pub struct Encoders {
    pub h264: String,
    pub hevc: String,
}

/// 编码器的检测结果(配置不变时不重复检测)
static ENCODERS: Mutex<Option<(String, Encoders)>> = Mutex::new(None);

/// 获取要使用的编码器
/// 自动选择时依次尝试 VideoToolbox、NVENC、QSV,用一个很短的视频实际编码一次,能成功才使用,都不行时使用软件编码
pub async fn encoders() -> Encoders {
    let config = config_service::get();
    let key = format!("{}|{}", super::ffmpeg_util::ffmpeg(), config.video_encoder);
    if let Some((cached_key, it)) = ENCODERS.lock().unwrap().as_ref()
        && *cached_key == key
    {
        return it.clone();
    }
    let candidates: Vec<&str> = match config.video_encoder.as_str() {
        "software" => vec![],
        "" => vec!["videotoolbox", "nvenc", "qsv"],
        it => vec![it],
    };
    let mut h264 = "libx264".to_string();
    let mut hevc = "libx265".to_string();
    for name in candidates {
        let (h, v) = (format!("h264_{}", name), format!("hevc_{}", name));
        if test_encoder(&h).await && test_encoder(&v).await {
            (h264, hevc) = (h, v);
            break;
        }
    }
    let encoders = Encoders { h264, hevc };
    *ENCODERS.lock().unwrap() = Some((key, encoders.clone()));
    encoders
}

/// 用一个很短的视频测试编码器能不能使用
async fn test_encoder(encoder: &str) -> bool {
    let args = [
        "-v", "error", "-f", "lavfi", "-i", "color=c=black:s=1280x720:d=0.2", "-c:v", encoder, "-pix_fmt", "yuv420p",
        "-f", "null", "-",
    ];
    run(&ffmpeg(), &args, Duration::from_secs(20)).await.is_ok()
}

/// 转码使用的线程数
fn threads() -> String {
    let threads = config_service::get().video_threads;
    let threads = if threads > 0 {
        threads as usize
    } else {
        let cpus = std::thread::available_parallelism().map(|it| it.get()).unwrap_or(2);
        (cpus / 2).max(1)
    };
    threads.to_string()
}

/// 硬件编码按分辨率设置比特率(kbps),软件编码用CRF控制画质
fn hardware_bit_rate(short_side: i64, is_hevc: bool) -> i64 {
    let h264 = match short_side {
        ..=480 => 1500,
        ..=720 => 3000,
        ..=1080 => 6000,
        ..=1440 => 12000,
        _ => 24000,
    };
    //HEVC同样画质大约只需要H.264的60%
    if is_hevc { h264 * 6 / 10 } else { h264 }
}

/// 视频编码参数
/// - source_bit_rate 源文件的比特率(kbps),源文件码率很低时(画面简单、AV1压缩率高)不需要用很高的码率
fn video_args(encoder: &str, short_side: i64, source_bit_rate: i64) -> Vec<String> {
    let is_hevc = encoder.starts_with("hevc") || encoder == "libx265";

    //目标码率不超过源文件的2倍(H.264、HEVC的压缩率比VP9、AV1低)
    let mut rate = hardware_bit_rate(short_side, is_hevc);
    if source_bit_rate > 0 {
        rate = rate.min((source_bit_rate * 2).max(500));
    }
    let mut args: Vec<String> = vec!["-c:v".into(), encoder.into()];
    match encoder {
        "libx264" => args.extend(["-preset", "veryfast", "-crf", "23"].map(String::from)),
        "libx265" => {
            //x265用线程池参数控制线程数
            args.extend(["-preset", "fast", "-crf", "24"].map(String::from));
            args.extend(["-x265-params".into(), format!("pools={}:log-level=error", threads())]);
        }
        _ => args.extend(["-b:v".into(), format!("{}k", rate)]),
    }

    //软件编码用CRF控制画质,同时限制最大码率
    args.extend(["-maxrate".into(), format!("{}k", rate * 3 / 2), "-bufsize".into(), format!("{}k", rate * 2)]);

    //硬件编码器只设置pix_fmt时可能输出10位(Main10),同时指定profile
    args.extend(["-pix_fmt", "yuv420p", "-profile:v", if is_hevc { "main" } else { "high" }].map(String::from));
    if is_hevc {
        args.extend(["-tag:v", "hvc1"].map(String::from));
    }
    args
}

/// 转码成原画:能直接播放的编码只重新封装,其他编码转码成同分辨率的HEVC
pub async fn transcode_original(source: &Path, target: &Path, info: &VideoInfo) -> Result<(), String> {
    let source = source.to_string_lossy().to_string();
    let target = target.to_string_lossy().to_string();
    let mut args: Vec<String> = ["-v", "error", "-y", "-i", &source, "-map", "0:v:0", "-map", "0:a:0?", "-sn", "-dn", "-map_metadata", "-1"]
        .map(String::from)
        .to_vec();
    if info.is_compatible() {
        args.extend(["-c", "copy"].map(String::from));
        if info.codec == "hevc" {
            args.extend(["-tag:v", "hvc1"].map(String::from));
        }
    } else {
        let encoders = encoders().await;
        args.extend(["-threads".into(), threads()]);
        args.extend(video_args(&encoders.hevc, info.short_side(), info.bit_rate));
        args.extend(audio_args(info, 192));
    }
    args.extend(["-movflags", "+faststart", "-f", "mp4"].map(String::from));
    args.push(target);
    let args: Vec<&str> = args.iter().map(|it| it.as_str()).collect();
    run(&ffmpeg(), &args, TRANSCODE_TIMEOUT).await?;
    Ok(())
}

/// 是否需要转码流畅版:短边超过480或者帧率超过25
pub fn need_smooth(info: &VideoInfo) -> bool {
    info.short_side() > SMOOTH_SIZE || info.frame_rate > SMOOTH_FPS
}

/// 转码成流畅版:短边超过480时缩小到480(不放大),帧率超过25时降到25,H.264编码
pub async fn transcode_smooth(source: &Path, target: &Path, info: &VideoInfo) -> Result<(), String> {
    let source = source.to_string_lossy().to_string();
    let target = target.to_string_lossy().to_string();
    let encoders = encoders().await;

    let mut filters = Vec::new();
    if info.short_side() > SMOOTH_SIZE {
        //横屏的视频按高缩放,竖屏的按宽缩放,另一边按比例(取偶数)
        filters.push(if info.width >= info.height {
            format!("scale=-2:{}", SMOOTH_SIZE)
        } else {
            format!("scale={}:-2", SMOOTH_SIZE)
        });
    }
    if info.frame_rate > SMOOTH_FPS {
        filters.push(format!("fps={}", SMOOTH_FPS));
    }
    let mut args: Vec<String> = ["-v", "error", "-y", "-i", &source, "-map", "0:v:0", "-map", "0:a:0?", "-sn", "-dn", "-map_metadata", "-1"]
        .map(String::from)
        .to_vec();
    args.extend(["-threads".into(), threads()]);
    if !filters.is_empty() {
        args.extend(["-vf".into(), filters.join(",")]);
    }
    args.extend(video_args(&encoders.h264, info.short_side().min(SMOOTH_SIZE), info.bit_rate));
    args.extend(audio_args(info, 128));
    args.extend(["-movflags", "+faststart", "-f", "mp4"].map(String::from));
    args.push(target);
    let args: Vec<&str> = args.iter().map(|it| it.as_str()).collect();
    run(&ffmpeg(), &args, TRANSCODE_TIMEOUT).await?;
    Ok(())
}

/// 音频参数:AAC直接复制,其他编码(如opus)转成AAC
fn audio_args(info: &VideoInfo, bit_rate: i64) -> Vec<String> {
    match info.audio_codec.as_deref() {
        None => vec![],
        Some("aac") => ["-c:a", "copy"].map(String::from).to_vec(),
        Some(_) => vec!["-c:a".into(), "aac".into(), "-b:a".into(), format!("{}k", bit_rate), "-ac".into(), "2".into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(width: i64, height: i64, frame_rate: i64) -> VideoInfo {
        VideoInfo { width, height, frame_rate, ..Default::default() }
    }

    /// 超过480p或25fps才转码流畅版
    #[test]
    fn need_smooth_rule() {
        assert!(!need_smooth(&info(854, 480, 25)), "480p 25fps 本身就是流畅版");
        assert!(!need_smooth(&info(640, 360, 24)));
        assert!(need_smooth(&info(854, 480, 30)));
        assert!(need_smooth(&info(1138, 640, 30)));
        assert!(need_smooth(&info(1280, 720, 30)));
        assert!(need_smooth(&info(1280, 720, 60)));
        assert!(need_smooth(&info(1920, 1080, 30)));
        assert!(need_smooth(&info(1920, 1080, 60)));
        assert!(need_smooth(&info(3840, 2160, 30)));
        assert!(need_smooth(&info(3840, 2160, 60)));
        assert!(need_smooth(&info(1080, 1920, 30)), "竖屏按宽算短边");
        assert!(!need_smooth(&info(480, 854, 25)));
    }

    /// 用环境变量 DAIRO_MUSIC_VIDEO_TEST_DIR 指定测试视频所在目录(static.mp4、motion.mp4),没有设置时跳过
    #[tokio::test]
    async fn video_pipeline() {
        let Ok(dir) = std::env::var("DAIRO_MUSIC_VIDEO_TEST_DIR") else {
            return;
        };
        let dir = Path::new(&dir);
        assert!(is_static(&dir.join("static.mp4")).await.unwrap(), "静态图片的视频应判断为没有视频");
        let motion = dir.join("motion.mp4");
        assert!(!is_static(&motion).await.unwrap(), "动态视频不应判断为只有图片");
        let info = probe(&motion).await.unwrap();
        println!("{:?} encoders={:?}", info, encoders().await);
        assert!(!info.is_compatible());

        let smooth = dir.join("out-smooth.mp4");
        let start = std::time::Instant::now();
        transcode_smooth(&motion, &smooth, &info).await.unwrap();
        let smooth_info = probe(&smooth).await.unwrap();
        println!("smooth {:?} {}ms", smooth_info, start.elapsed().as_millis());
        assert_eq!(smooth_info.codec, "h264");
        assert_eq!(smooth_info.short_side(), info.short_side().min(SMOOTH_SIZE));
        assert!(smooth_info.frame_rate <= SMOOTH_FPS);
        assert_eq!(smooth_info.audio_codec.as_deref(), Some("aac"));

        let original = dir.join("out-original.mp4");
        let start = std::time::Instant::now();
        transcode_original(&motion, &original, &info).await.unwrap();
        let original_info = probe(&original).await.unwrap();
        println!("original {:?} {}ms", original_info, start.elapsed().as_millis());
        assert_eq!(original_info.codec, "hevc");
        assert_eq!(original_info.height, 1440);
        assert_eq!(original_info.pix_fmt, "yuv420p");

        //已经兼容的文件只重新封装
        let remux = dir.join("out-remux.mp4");
        transcode_original(&original, &remux, &original_info).await.unwrap();
        assert_eq!(probe(&remux).await.unwrap().codec, "hevc");
    }
}
