//! 依赖软件的安装:内置的DairoDFS、DairoMUSIC-Search,以及ffmpeg、ffprobe、yt-dlp、deno
//! - 安装方式:后台上传安装包,或者填写URL下载
//! - 安装包可以是程序文件本身,也可以是zip、tar.gz、tar.xz压缩包(从里面找到程序文件)
//! - 安装位置:{数据目录}/bin/{软件}/{程序文件},重新安装就是替换,用来更新版本
//! - 替换之前先执行一次新程序获取版本号,不能运行的安装包不会替换已经安装的程序

use crate::service::{config_service, process_service};
use crate::util::time_util;
use hyperoute_core::BodyStream;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// 一个依赖软件
#[derive(Debug, PartialEq, Eq)]
pub struct Software {
    // 标识,也是安装目录名
    pub key: &'static str,

    // 显示名称
    pub name: &'static str,

    // 程序文件名(Windows上再加.exe)
    file_name: &'static str,

    // 获取版本号的参数
    version_args: &'static [&'static str],

    // 版本号在输出的最后一行(否则在第一行)
    version_last_line: bool,

    // 是否是需要一直运行的服务
    pub is_service: bool,
}

pub static DFS: Software = Software {
    key: "dfs",
    name: "DairoDFS",
    file_name: "DairoDFS",
    version_args: &["--version"],
    version_last_line: true,
    is_service: true,
};

pub static SEARCH: Software = Software {
    key: "search",
    name: "DairoMUSIC-Search",
    file_name: "dairo-music-search",
    version_args: &["--version"],
    version_last_line: true,
    is_service: true,
};

pub static FFMPEG: Software = Software {
    key: "ffmpeg",
    name: "ffmpeg",
    file_name: "ffmpeg",
    version_args: &["-version"],
    version_last_line: false,
    is_service: false,
};

pub static FFPROBE: Software = Software {
    key: "ffprobe",
    name: "ffprobe",
    file_name: "ffprobe",
    version_args: &["-version"],
    version_last_line: false,
    is_service: false,
};

pub static YT_DLP: Software = Software {
    key: "yt-dlp",
    name: "yt-dlp",
    file_name: "yt-dlp",
    version_args: &["--version"],
    version_last_line: false,
    is_service: false,
};

pub static DENO: Software = Software {
    key: "deno",
    name: "deno",
    file_name: "deno",
    version_args: &["--version"],
    version_last_line: false,
    is_service: false,
};

/// 所有的依赖软件
pub static ALL: [&Software; 6] = [&DFS, &SEARCH, &FFMPEG, &FFPROBE, &YT_DLP, &DENO];

/// 通过标识获取软件
pub fn find(key: &str) -> Option<&'static Software> {
    ALL.iter().copied().find(|it| it.key == key)
}

/// 程序文件名
fn exe_name(sw: &Software) -> String {
    if cfg!(windows) {
        format!("{}.exe", sw.file_name)
    } else {
        sw.file_name.to_string()
    }
}

/// 安装目录 {数据目录}/bin/{软件}
pub fn dir(sw: &Software) -> PathBuf {
    config_service::data_dir().join("bin").join(sw.key)
}

/// 内置程序的路径
pub fn install_path(sw: &Software) -> PathBuf {
    dir(sw).join(exe_name(sw))
}

/// 是否已经安装了内置的程序
pub fn is_installed(sw: &Software) -> bool {
    install_path(sw).is_file()
}

/// 后台填写的外部程序路径(只有ffmpeg等工具可以填写)
pub fn external_path(sw: &Software) -> String {
    let config = config_service::get();
    match sw.key {
        "ffmpeg" => config.ffmpeg_path,
        "ffprobe" => config.ffprobe_path,
        "yt-dlp" => config.yt_dlp_path,
        "deno" => config.deno_path,
        _ => String::new(),
    }
}

/// 要使用的程序:安装了内置的就用内置的,否则用后台填写的路径,都没有时用程序名(从PATH查找)
pub fn program(sw: &Software) -> String {
    if is_installed(sw) {
        return install_path(sw).to_string_lossy().to_string();
    }
    let path = external_path(sw);
    if path.is_empty() { sw.file_name.to_string() } else { path }
}

/// 安装状态
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallState {
    // 是否正在安装
    pub running: bool,

    // 已经下载(上传)的大小
    pub downloaded: u64,

    // 文件总大小,不知道时为0
    pub total: u64,

    // 最近一次安装失败的原因,成功时为空
    pub error: String,

    // 安装日志
    pub logs: Vec<String>,
}

/// 各个软件的安装状态
static STATES: LazyLock<Mutex<HashMap<&'static str, InstallState>>> = LazyLock::new(Default::default);

/// 程序路径 -> 版本号(或者不能运行的原因),同一个程序不重复执行
static VERSIONS: LazyLock<Mutex<HashMap<String, Result<String, String>>>> = LazyLock::new(Default::default);

/// 获取安装状态(重启后从日志文件里读取上一次安装的日志)
pub fn state(sw: &'static Software) -> InstallState {
    let mut states = STATES.lock().unwrap();
    states
        .entry(sw.key)
        .or_insert_with(|| {
            let logs = std::fs::read_to_string(dir(sw).join("install.log")).unwrap_or_default();
            InstallState {
                logs: logs.lines().map(|it| it.to_string()).collect(),
                ..Default::default()
            }
        })
        .clone()
}

/// 修改安装状态
fn update_state(sw: &'static Software, f: impl FnOnce(&mut InstallState)) {
    state(sw);
    let mut states = STATES.lock().unwrap();
    f(states.get_mut(sw.key).unwrap());
}

/// 记录安装日志
fn log(sw: &'static Software, msg: impl Into<String>) {
    let msg = msg.into();
    println!("-->[安装{}] {}", sw.name, msg);
    let line = format!("[{}] {}", time_util::now_text(), msg);
    update_state(sw, |it| it.logs.push(line));
}

/// 开始安装,已经在安装中时返回错误
fn begin(sw: &'static Software) -> Result<(), String> {
    state(sw);
    let mut states = STATES.lock().unwrap();
    let state = states.get_mut(sw.key).unwrap();
    if state.running {
        return Err(format!("{}正在安装中,请等待安装完成", sw.name));
    }
    *state = InstallState {
        running: true,
        ..Default::default()
    };
    Ok(())
}

/// 安装结束,保存安装日志
fn finish(sw: &'static Software, result: Result<String, String>) {
    match result {
        Ok(version) => log(sw, format!("安装完成,版本: {}", version)),
        Err(e) => {
            log(sw, format!("安装失败: {}", e));
            update_state(sw, |it| it.error = e);
        }
    }
    update_state(sw, |it| it.running = false);
    let logs = state(sw).logs.join("\n");
    let _ = std::fs::create_dir_all(dir(sw));
    let _ = std::fs::write(dir(sw).join("install.log"), logs);
}

/// 下载用的HTTP客户端
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(60))
        .user_agent(concat!("DairoMUSIC/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap()
});

/// 通过URL下载安装(后台执行,通过state查看进度)
pub fn install_by_url(sw: &'static Software, url: String) -> Result<(), String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("下载地址必须以http://或https://开头".to_string());
    }
    begin(sw)?;
    tokio::spawn(async move {
        let result = async {
            log(sw, format!("开始下载: {}", url));
            let file = dir(sw).join("download.tmp");
            download(sw, &url, &file).await?;
            let name = url.split(['?', '#']).next().unwrap_or_default().rsplit('/').next().unwrap_or_default().to_string();
            let result = install_package(sw, &file, &name).await;
            let _ = tokio::fs::remove_file(&file).await;
            result
        }
        .await;
        finish(sw, result);
    });
    Ok(())
}

/// 下载文件
async fn download(sw: &'static Software, url: &str, file: &Path) -> Result<(), String> {
    tokio::fs::create_dir_all(dir(sw)).await.map_err(|e| e.to_string())?;
    let mut res = CLIENT.get(url).send().await.map_err(|e| format!("下载失败: {}", e))?;
    if !res.status().is_success() {
        return Err(format!("下载失败: HTTP {}", res.status()));
    }
    let total = res.content_length().unwrap_or(0);
    update_state(sw, |it| it.total = total);
    let mut out = tokio::fs::File::create(file).await.map_err(|e| e.to_string())?;
    let mut downloaded = 0u64;
    loop {
        let chunk = match res.chunk().await {
            Ok(Some(it)) => it,
            Ok(None) => break,
            Err(e) => return Err(format!("下载中断: {}", e)),
        };
        out.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        update_state(sw, |it| it.downloaded = downloaded);
    }
    out.flush().await.map_err(|e| e.to_string())?;
    log(sw, format!("下载完成,大小: {}", size_text(downloaded)));
    Ok(())
}

/// 上传安装包(请求体直接是文件内容),上传完成后在后台安装
/// - name 上传的文件名
pub async fn install_by_upload(sw: &'static Software, name: String, mut body: BodyStream) -> Result<(), String> {
    begin(sw)?;
    log(sw, format!("开始上传: {}", name));
    update_state(sw, |it| it.total = body.size_hint().unwrap_or(0));
    let file = dir(sw).join("upload.tmp");
    let result = async {
        tokio::fs::create_dir_all(dir(sw)).await.map_err(|e| e.to_string())?;
        let mut out = tokio::fs::File::create(&file).await.map_err(|e| e.to_string())?;
        let mut size = 0u64;
        while let Some(chunk) = body.next_chunk().await {
            let chunk = chunk.map_err(|e| format!("上传中断: {}", e))?;
            out.write_all(&chunk).await.map_err(|e| e.to_string())?;
            size += chunk.len() as u64;
            update_state(sw, |it| it.downloaded = size);
        }
        out.flush().await.map_err(|e| e.to_string())?;
        if size == 0 {
            return Err("上传的文件是空的".to_string());
        }
        log(sw, format!("上传完成,大小: {}", size_text(size)));
        Ok(())
    }
    .await;
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&file).await;
        finish(sw, Err(e.clone()));
        return Err(e);
    }
    tokio::spawn(async move {
        let result = install_package(sw, &file, &name).await;
        let _ = tokio::fs::remove_file(&file).await;
        finish(sw, result);
    });
    Ok(())
}

/// 安装包类型
#[derive(Debug, PartialEq)]
enum PackageKind {
    // 程序文件本身
    Raw,
    Zip,

    // tar包(包括gz、xz、bz2压缩的)
    Tar,
}

/// 通过文件头判断安装包类型
fn package_kind(file: &Path) -> Result<PackageKind, String> {
    use std::io::Read;
    let mut head = [0u8; 512];
    let mut f = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let len = f.read(&mut head).map_err(|e| e.to_string())?;
    let head = &head[..len];
    Ok(if head.starts_with(b"PK\x03\x04") {
        PackageKind::Zip
    } else if head.starts_with(&[0x1f, 0x8b]) || head.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0]) || head.starts_with(b"BZh") {
        PackageKind::Tar
    } else if head.len() >= 262 && &head[257..262] == b"ustar" {
        PackageKind::Tar
    } else {
        PackageKind::Raw
    })
}

/// 安装一个安装包
/// - file 下载或上传的文件
/// - name 原来的文件名(只用来显示)
async fn install_package(sw: &'static Software, file: &Path, name: &str) -> Result<String, String> {
    let kind = package_kind(file)?;
    if kind == PackageKind::Raw {
        return install_program(sw, file).await;
    }
    log(sw, format!("解压安装包: {}", name));
    let work = dir(sw).join("work");
    let _ = tokio::fs::remove_dir_all(&work).await;
    tokio::fs::create_dir_all(&work).await.map_err(|e| e.to_string())?;
    let result = async {
        if kind == PackageKind::Zip {
            let (file, work) = (file.to_path_buf(), work.clone());
            tokio::task::spawn_blocking(move || unzip(&file, &work)).await.map_err(|e| e.to_string())??;
        } else {
            untar(file, &work).await?;
        }
        let files = list_files(&work);
        let program = files
            .iter()
            .find(|it| it.file_name().is_some_and(|name| name.to_string_lossy() == exe_name(sw)))
            .or(if files.len() == 1 { files.first() } else { None })
            .ok_or_else(|| format!("安装包里没有找到程序文件{}", exe_name(sw)))?;
        let version = install_program(sw, program).await?;

        //ffmpeg的安装包里一般也有ffprobe,一起安装
        if sw == &FFMPEG {
            let ffprobe = files.iter().find(|it| it.file_name().is_some_and(|name| name.to_string_lossy() == exe_name(&FFPROBE)));
            if let Some(ffprobe) = ffprobe {
                log(sw, "安装包里有ffprobe,一起安装");
                match install_program(&FFPROBE, ffprobe).await {
                    Ok(version) => log(sw, format!("ffprobe安装完成,版本: {}", version)),
                    Err(e) => log(sw, format!("ffprobe安装失败: {}", e)),
                }
            }
        }
        Ok(version)
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&work).await;
    result
}

/// 安装程序文件:检查能否运行,然后替换已经安装的程序
async fn install_program(sw: &'static Software, file: &Path) -> Result<String, String> {
    set_executable(file)?;
    log(sw, format!("检查程序能否运行: {}", sw.name));
    let version = run_version(sw, file).await.map_err(|e| {
        format!(
            "程序不能运行,请确认安装包是当前系统({} {})的版本。{}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            e
        )
    })?;
    log(sw, format!("版本: {}", version));

    //服务正在运行时先停止,替换之后再启动
    let was_running = sw.is_service && process_service::is_running(sw);
    if was_running {
        log(sw, format!("停止{}", sw.name));
        process_service::stop(sw).await;
    }
    let target = install_path(sw);
    tokio::fs::create_dir_all(dir(sw)).await.map_err(|e| e.to_string())?;
    if let Err(e) = tokio::fs::rename(file, &target).await {
        //不在同一个文件系统时复制
        tokio::fs::copy(file, &target).await.map_err(|_| format!("保存程序文件失败: {}", e))?;
        set_executable(&target)?;
    }
    clear_version_cache();
    if sw.is_service && (was_running || process_service::should_run(sw)) {
        log(sw, format!("启动{}", sw.name));
        process_service::start(sw);
    }
    Ok(version)
}

/// 卸载内置的程序
pub async fn uninstall(sw: &'static Software) -> Result<(), String> {
    if state(sw).running {
        return Err(format!("{}正在安装中", sw.name));
    }
    if sw.is_service {
        process_service::stop(sw).await;
    }
    let path = install_path(sw);
    if path.exists() {
        tokio::fs::remove_file(&path).await.map_err(|e| e.to_string())?;
    }
    clear_version_cache();
    log(sw, "已卸载");
    let logs = state(sw).logs.join("\n");
    let _ = std::fs::write(dir(sw).join("install.log"), logs);
    Ok(())
}

/// 程序改变之后清除版本号的缓存
pub fn clear_version_cache() {
    VERSIONS.lock().unwrap().clear();
}

/// 当前使用的程序的版本号
pub async fn version(sw: &'static Software) -> Result<String, String> {
    let program = program(sw);
    if let Some(it) = VERSIONS.lock().unwrap().get(&program) {
        return it.clone();
    }
    let result = run_version(sw, Path::new(&program)).await;
    VERSIONS.lock().unwrap().insert(program, result.clone());
    result
}

/// 执行程序获取版本号
async fn run_version(sw: &Software, program: &Path) -> Result<String, String> {
    let child = Command::new(program)
        .args(sw.version_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    let output = match tokio::time::timeout(Duration::from_secs(20), child).await {
        Ok(Ok(it)) => it,
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => return Err("没有找到程序".to_string()),
        Ok(Err(e)) => return Err(e.to_string()),
        Err(_) => return Err("获取版本号超时".to_string()),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        let msg = stderr.trim();
        return Err(if msg.is_empty() { format!("执行失败({})", output.status) } else { msg.chars().take(500).collect() });
    }
    let text = if stdout.trim().is_empty() { &stderr } else { &stdout };
    let mut lines = text.lines().map(|it| it.trim()).filter(|it| !it.is_empty());
    let line = if sw.version_last_line { lines.last() } else { lines.next() };
    let line = line.unwrap_or_default();

    //ffmpeg的版本信息后面是版权信息,去掉
    let line = line.split(" Copyright").next().unwrap_or_default().trim();
    Ok(line.to_string())
}

/// 设置可执行权限
fn set_executable(file: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

/// 解压zip包
fn unzip(file: &Path, target: &Path) -> Result<(), String> {
    use std::io::Read;
    let f = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(f).map_err(|e| format!("解压失败: {}", e))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("解压失败: {}", e))?;

        //防止压缩包里的路径跑到目标目录外面
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let out = target.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| format!("解压失败: {}", e))?;
        std::fs::write(&out, data).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 解压tar包(使用系统的tar命令,支持gz、xz、bz2)
async fn untar(file: &Path, target: &Path) -> Result<(), String> {
    let output = Command::new("tar")
        .arg("-xf")
        .arg(file)
        .arg("-C")
        .arg(target)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("解压失败,系统里没有tar命令: {}", e))?;
    if !output.status.success() {
        return Err(format!("解压失败: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(())
}

/// 列出目录下所有的文件(包括子目录里的)
fn list_files(dir: &Path) -> Vec<PathBuf> {
    let mut list = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return list;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            list.extend(list_files(&path));
        } else if meta.is_file() {
            list.push(path);
        }
    }
    list
}

/// 文件大小的显示文本
fn size_text(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1}MB", size as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1}KB", size as f64 / 1024.0)
    }
}

/// 推荐的下载地址
#[derive(Debug, Serialize)]
pub struct Recommend {
    pub url: String,

    // 版本号,不知道时为空
    pub version: String,
}

/// 获取当前系统推荐的下载地址
pub async fn recommend(sw: &'static Software) -> Result<Recommend, String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let unsupported = || format!("没有当前系统({} {})的推荐下载地址,请手动上传安装包", os, arch);
    let latest = |url: String| Recommend {
        url,
        version: String::new(),
    };
    match sw.key {
        "dfs" => {
            let platform = match (os, arch) {
                ("linux", "x86_64") => "linux-amd64",
                ("linux", "aarch64") => "linux-arm64",
                ("macos", "aarch64") => "mac-darwin-aarch64",
                ("macos", "x86_64") => "mac-darwin-amd64",
                ("windows", "x86_64") => "win-amd64",
                _ => return Err(unsupported()),
            };
            github_latest("DAIRO-HY/DairoDFS", |name| name.starts_with(&format!("DairoDFS-{}-", platform))).await
        }
        "search" => {
            let platform = match (os, arch) {
                ("linux", "x86_64") => "linux-amd64",
                ("linux", "aarch64") => "linux-arm64",
                ("macos", "aarch64") => "mac-arm64",
                ("macos", "x86_64") => "mac-amd64",
                ("windows", "x86_64") => "win-amd64.exe",
                ("windows", "aarch64") => "win-arm64.exe",
                _ => return Err(unsupported()),
            };
            github_latest("DAIRO-HY/DairoMUSIC-Search", |name| name == format!("dairo-music-search-{}", platform)).await
        }
        "ffmpeg" | "ffprobe" => {
            let platform = match (os, arch) {
                ("linux", "x86_64") => "linux-amd64",
                ("windows", _) => "win",
                ("macos", _) => "macOS",
                _ => return Err(unsupported()),
            };
            Ok(Recommend {
                url: format!("https://github.com/DAIRO-HY/DairoDfsLib/raw/main/{}-7.0.2-{}.zip", sw.key, platform),
                version: "7.0.2".to_string(),
            })
        }
        "yt-dlp" => {
            let file = match (os, arch) {
                ("macos", _) => "yt-dlp_macos",
                ("linux", "x86_64") => "yt-dlp_linux",
                ("linux", "aarch64") => "yt-dlp_linux_aarch64",
                ("windows", _) => "yt-dlp.exe",
                _ => return Err(unsupported()),
            };
            Ok(latest(format!("https://github.com/yt-dlp/yt-dlp/releases/latest/download/{}", file)))
        }
        "deno" => {
            let target = match (os, arch) {
                ("macos", "aarch64") => "aarch64-apple-darwin",
                ("macos", "x86_64") => "x86_64-apple-darwin",
                ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
                ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
                ("windows", "x86_64") => "x86_64-pc-windows-msvc",
                _ => return Err(unsupported()),
            };
            Ok(latest(format!("https://github.com/denoland/deno/releases/latest/download/deno-{}.zip", target)))
        }
        _ => Err(unsupported()),
    }
}

/// 从GitHub获取最新发布版本里的安装包地址
/// - repo 仓库,如 DAIRO-HY/DairoDFS
/// - matches 判断是不是当前系统的安装包(按文件名)
async fn github_latest(repo: &str, matches: impl Fn(&str) -> bool) -> Result<Recommend, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let res = CLIENT
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("获取最新版本失败: {}", e))?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!("{}还没有发布可以下载的版本,请手动上传安装包", repo));
    }
    if !res.status().is_success() {
        return Err(format!("获取最新版本失败: HTTP {}", res.status()));
    }
    let release: serde_json::Value = res.json().await.map_err(|e| format!("获取最新版本失败: {}", e))?;
    let version = release["tag_name"].as_str().unwrap_or_default().to_string();
    let assets = release["assets"].as_array().cloned().unwrap_or_default();
    let asset = assets
        .iter()
        .find(|it| matches(it["name"].as_str().unwrap_or_default()))
        .ok_or_else(|| format!("最新版本{}里没有当前系统({} {})的安装包", version, std::env::consts::OS, std::env::consts::ARCH))?;
    Ok(Recommend {
        url: asset["browser_download_url"].as_str().unwrap_or_default().to_string(),
        version,
    })
}
