//! 调用ffmpeg/ffprobe处理歌曲文件
//! 直接执行程序并逐个传参,不经过shell,文件名里有特殊字符也没有问题

use crate::service::software_service;
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

/// 转码的超时时间
const TRANSCODE_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// 读取信息的超时时间
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// 歌曲文件信息
#[derive(Debug, Default, Clone)]
pub struct AudioInfo {
    // 文件格式 mp3、flac、wav、ape、m4a、ogg、wma等
    pub format: String,

    // 时长(毫秒)
    pub duration: i64,

    // 比特率(kbps)
    pub bit_rate: i64,

    // 采样率
    pub sample_rate: Option<i64>,

    // 歌名、歌手、专辑、年代(文件里的标签)
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i64>,

    // 是否有封面图片
    pub has_cover: bool,
}

/// ffmpeg程序(安装了内置的ffmpeg时使用内置的)
pub(crate) fn ffmpeg() -> String {
    software_service::program(&software_service::FFMPEG)
}

/// ffprobe程序
pub(crate) fn ffprobe() -> String {
    software_service::program(&software_service::FFPROBE)
}

/// 执行命令,返回标准输出;失败时返回标准错误的内容
pub(crate) async fn run(program: &str, args: &[&str], timeout: Duration) -> Result<Vec<u8>, String> {
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        //超时或者任务被取消时结束进程
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("无法执行{}: {}", program, e))?;
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| format!("{}执行超时", program))?
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);

        //错误信息可能很长,只保留最后一部分
        let err: String = err.chars().rev().take(500).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(format!("{}执行失败: {}", program, err.trim()));
    }
    Ok(output.stdout)
}

/// 获取版本信息(检测是否可以使用)
pub async fn version(is_probe: bool) -> Result<String, String> {
    let program = if is_probe { ffprobe() } else { ffmpeg() };
    let out = run(&program, &["-version"], Duration::from_secs(10)).await?;
    let text = String::from_utf8_lossy(&out);
    Ok(text.lines().next().unwrap_or_default().to_string())
}

/// 读取歌曲文件信息
pub async fn probe(path: &Path) -> Result<AudioInfo, String> {
    let path = path.to_string_lossy();
    let out = run(
        &ffprobe(),
        &["-v", "error", "-print_format", "json", "-show_format", "-show_streams", &path],
        PROBE_TIMEOUT,
    )
    .await?;
    let json: Value = serde_json::from_slice(&out).map_err(|e| format!("解析歌曲信息失败: {}", e))?;
    let streams = json["streams"].as_array().cloned().unwrap_or_default();
    let Some(audio) = streams.iter().find(|it| it["codec_type"] == "audio") else {
        return Err("不是歌曲文件(没有音频)".to_string());
    };
    let format = &json["format"];
    let number = |value: &Value| -> Option<f64> {
        value.as_str().and_then(|it| it.parse::<f64>().ok()).or_else(|| value.as_f64())
    };
    let duration = number(&format["duration"]).or_else(|| number(&audio["duration"])).unwrap_or(0.0);
    if duration <= 0.0 {
        return Err("无法获取歌曲时长".to_string());
    }

    //有损格式取音频流的比特率,无损格式音频流里可能没有,取整个文件的比特率
    let bit_rate = number(&audio["bit_rate"]).or_else(|| number(&format["bit_rate"])).unwrap_or(0.0);
    let tags = merge_tags(&format["tags"], &audio["tags"]);
    let tag = |name: &str| -> Option<String> {
        tags.iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim().to_string())
            //乱码(???)和空字符串不使用
            .filter(|it| !it.is_empty() && !it.contains("??"))
    };
    let year = tag("date")
        .or_else(|| tag("year"))
        .and_then(|it| it.get(..4).and_then(|y| y.parse::<i64>().ok()));
    let has_cover = streams
        .iter()
        .any(|it| it["codec_type"] == "video" && it["disposition"]["attached_pic"] == 1);
    Ok(AudioInfo {
        format: file_format(format["format_name"].as_str().unwrap_or_default(), audio["codec_name"].as_str().unwrap_or_default()),
        duration: (duration * 1000.0) as i64,
        bit_rate: (bit_rate / 1000.0).round() as i64,
        sample_rate: number(&audio["sample_rate"]).map(|it| it as i64),
        title: tag("title"),
        artist: tag("artist").or_else(|| tag("album_artist")),
        album: tag("album"),
        year,
        has_cover,
    })
}

/// 合并文件和音频流的标签(有的格式标签在音频流里)
fn merge_tags(format_tags: &Value, stream_tags: &Value) -> Vec<(String, String)> {
    let mut list = Vec::new();
    for tags in [format_tags, stream_tags] {
        if let Some(map) = tags.as_object() {
            for (key, value) in map {
                if let Some(value) = value.as_str() {
                    list.push((key.clone(), value.to_string()));
                }
            }
        }
    }
    list
}

/// 根据ffprobe的格式名得到文件格式(扩展名)
fn file_format(format_name: &str, codec: &str) -> String {
    let names: Vec<&str> = format_name.split(',').collect();
    let format = if names.contains(&"mp3") {
        "mp3"
    } else if names.contains(&"flac") {
        "flac"
    } else if names.contains(&"wav") {
        "wav"
    } else if names.contains(&"ape") {
        "ape"
    } else if names.contains(&"ogg") {
        "ogg"
    } else if names.contains(&"asf") {
        "wma"
    } else if names.contains(&"m4a") || names.contains(&"mp4") {
        "m4a"
    } else if codec == "aac" {
        "aac"
    } else {
        names.first().copied().unwrap_or("bin")
    };
    format.to_string()
}

/// 转码成mp3
/// - bit_rate 目标比特率(kbps)
pub async fn transcode_mp3(source: &Path, target: &Path, bit_rate: i64) -> Result<(), String> {
    let source = source.to_string_lossy();
    let target = target.to_string_lossy();
    let rate = format!("{}k", bit_rate);
    run(
        &ffmpeg(),
        &["-v", "error", "-y", "-i", &source, "-vn", "-map_metadata", "-1", "-ac", "2", "-b:a", &rate, "-f", "mp3", &target],
        TRANSCODE_TIMEOUT,
    )
    .await?;
    Ok(())
}

/// 获取文件里的封面图片(转换成JPG)
pub async fn cover(source: &Path) -> Result<Vec<u8>, String> {
    let source = source.to_string_lossy();
    let data = run(
        &ffmpeg(),
        &["-v", "error", "-i", &source, "-an", "-map", "0:v:0", "-frames:v", "1", "-c:v", "mjpeg", "-q:v", "2", "-f", "image2pipe", "-"],
        PROBE_TIMEOUT,
    )
    .await?;
    if data.is_empty() {
        return Err("没有封面图片".to_string());
    }
    Ok(data)
}
