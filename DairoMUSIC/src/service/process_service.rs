//! 内置服务(DairoDFS、DairoMUSIC-Search)的运行管理
//! - DairoMUSIC启动时一起启动,退出时一起停止
//! - 服务异常退出时自动重启,连续失败时等待时间逐渐变长(最长1分钟)
//! - 服务的输出保存在内存里,后台页面可以查看最近的日志

use crate::service::software_service::{self, Software, DFS, SEARCH};
use crate::service::{config_service, dfs_service};
use crate::util::time_util;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::watch;

/// 内存里最多保存的日志行数
const MAX_LOG_LINES: usize = 1000;

/// 服务的运行状态
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    // stopped:已停止 starting:启动中 running:运行中 waiting:异常退出,等待重启
    pub status: &'static str,

    // 进程ID
    pub pid: Option<u32>,

    // 这次启动的时间(毫秒)
    pub start_time: i64,

    // 异常退出后自动重启的次数
    pub restart_count: u32,

    // 提示信息,比如异常退出的原因、DFS初始化失败的原因
    pub message: String,
}

/// 一个内置服务
struct Service {
    // 是否需要运行
    desired: watch::Sender<bool>,
    info: Mutex<ServiceInfo>,
    logs: Mutex<VecDeque<String>>,
}

/// 所有的内置服务
static SERVICES: LazyLock<HashMap<&'static str, Arc<Service>>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for sw in [&DFS, &SEARCH] {
        let service = Arc::new(Service {
            desired: watch::Sender::new(false),
            info: Mutex::new(ServiceInfo {
                status: "stopped",
                ..Default::default()
            }),
            logs: Mutex::new(VecDeque::new()),
        });
        tokio::spawn(supervise(sw, service.clone()));
        map.insert(sw.key, service);
    }
    map
});

fn service(sw: &Software) -> &'static Arc<Service> {
    SERVICES.get(sw.key).expect("不是内置服务")
}

/// 启动时按配置启动内置服务
pub fn init() {
    for sw in [&DFS, &SEARCH] {
        if should_run(sw) {
            start(sw);
        }
    }
}

/// 按配置是否应该运行
pub fn should_run(sw: &Software) -> bool {
    let config = config_service::get();
    let enabled = match sw.key {
        "dfs" => config.dfs_builtin,
        "search" => config.collect_builtin && config.collect_enable,
        _ => false,
    };
    enabled && software_service::is_installed(sw)
}

/// 配置修改之后:应该运行的重新启动(使用新的配置),不应该运行的停止
pub async fn apply_config(sw: &'static Software) {
    if should_run(sw) {
        restart(sw).await;
    } else {
        stop(sw).await;
    }
}

/// 启动服务
pub fn start(sw: &Software) {
    service(sw).desired.send_replace(true);
}

/// 停止服务,等待进程退出
pub async fn stop(sw: &Software) {
    let service = service(sw);
    service.desired.send_replace(false);
    for _ in 0..300 {
        if service.info.lock().unwrap().status == "stopped" {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 重启服务
pub async fn restart(sw: &Software) {
    stop(sw).await;
    start(sw);
}

/// 停止所有的服务(DairoMUSIC退出时)
pub async fn stop_all() {
    for sw in [&DFS, &SEARCH] {
        if is_running(sw) {
            println!("-->停止{}", sw.name);
            stop(sw).await;
        }
    }
}

/// 是否正在运行(包括启动中、等待重启)
pub fn is_running(sw: &Software) -> bool {
    *service(sw).desired.borrow() || service(sw).info.lock().unwrap().status != "stopped"
}

/// 运行状态
pub fn info(sw: &Software) -> ServiceInfo {
    service(sw).info.lock().unwrap().clone()
}

/// 最近的日志
pub fn logs(sw: &Software) -> Vec<String> {
    service(sw).logs.lock().unwrap().iter().cloned().collect()
}

/// 记录一行日志
pub fn log(sw: &Software, line: impl AsRef<str>) {
    let line = line.as_ref();
    println!("[{}] {}", sw.name, line);
    let mut logs = service(sw).logs.lock().unwrap();
    logs.push_back(format!("[{}] {}", time_util::now_text(), line));
    while logs.len() > MAX_LOG_LINES {
        logs.pop_front();
    }
}

/// 设置提示信息
pub fn set_message(sw: &Software, message: impl Into<String>) {
    service(sw).info.lock().unwrap().message = message.into();
}

/// 修改运行状态
fn set_info(sw: &Software, f: impl FnOnce(&mut ServiceInfo)) {
    f(&mut service(sw).info.lock().unwrap());
}

/// 守护一个服务:需要运行时启动,异常退出时重启,不需要运行时停止
async fn supervise(sw: &'static Software, service: Arc<Service>) {
    let mut desired = service.desired.subscribe();

    //连续失败的次数
    let mut fail_count = 0u32;
    loop {
        //等待需要运行
        while !*desired.borrow_and_update() {
            set_info(sw, |it| {
                it.status = "stopped";
                it.pid = None;
            });
            fail_count = 0;
            if desired.changed().await.is_err() {
                return;
            }
        }
        set_info(sw, |it| {
            it.status = "starting";
            it.message = String::new();
        });
        let started = Instant::now();
        let exit = match spawn(sw).await {
            Ok(mut child) => {
                let pid = child.id();
                log(sw, format!("已启动,进程ID: {}", pid.unwrap_or_default()));
                set_info(sw, |it| {
                    it.status = "running";
                    it.pid = pid;
                    it.start_time = time_util::current_millis();
                });
                if let Some(pid) = pid {
                    let _ = std::fs::write(software_service::dir(sw).join("pid"), pid.to_string());
                }
                if sw == &DFS {
                    //第一次启动时自动初始化,并获取API票据
                    tokio::spawn(dfs_service::init_builtin());
                }

                //等待进程退出,或者收到停止的指令
                let exit = loop {
                    tokio::select! {
                        status = child.wait() => break Some(status.map(|it| it.to_string()).unwrap_or_else(|e| e.to_string())),
                        changed = desired.changed() => {
                            if changed.is_err() || !*desired.borrow_and_update() {
                                kill(sw, &mut child).await;
                                break None;
                            }
                        }
                    }
                };
                let _ = std::fs::remove_file(software_service::dir(sw).join("pid"));
                exit
            }
            Err(e) => Some(format!("启动失败: {}", e)),
        };
        let Some(exit) = exit else {
            log(sw, "已停止");
            continue;
        };

        //异常退出,稍后重启
        if started.elapsed() > Duration::from_secs(60) {
            fail_count = 0;
        }
        fail_count += 1;
        let delay = 2u64.pow(fail_count.min(6)).min(60);
        let message = format!("异常退出({}),{}秒后重启", exit, delay);
        log(sw, &message);
        set_info(sw, |it| {
            it.status = "waiting";
            it.pid = None;
            it.restart_count += 1;
            it.message = message;
        });
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(delay)) => {}
            changed = desired.changed() => {
                if changed.is_err() {
                    return;
                }
            }
        }
    }
}

/// 启动进程
async fn spawn(sw: &'static Software) -> Result<Child, String> {
    let program = software_service::install_path(sw);
    if !program.is_file() {
        return Err(format!("还没有安装{}", sw.name));
    }
    kill_orphan(sw, &program).await;
    let config = config_service::get();

    //端口被其他程序占用时不启动,避免把请求发给别的程序(比如自动初始化了别人的DFS)
    let port = if sw == &DFS { config_service::dfs_port(&config) } else { config_service::collect_port(&config) };
    if let Err(e) = std::net::TcpListener::bind(("0.0.0.0", port)) {
        return Err(format!("端口{}已经被其他程序占用,请在设置里换一个端口({})", port, e));
    }
    let mut cmd = Command::new(&program);
    if sw == &DFS {
        let data_dir = config_service::dfs_data_dir(&config);
        std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建目录{}失败: {}", data_dir.display(), e))?;
        cmd.arg("--web-port")
            .arg(config_service::dfs_port(&config).to_string())
            .arg("--data-dir")
            .arg(&data_dir)
            .current_dir(&data_dir);
    } else {
        let dir = config_service::collect_data_dir(&config);
        let (cache_dir, data_dir) = (dir.join("temp"), dir.join("data"));
        for it in [&cache_dir, &data_dir] {
            std::fs::create_dir_all(it).map_err(|e| format!("创建目录{}失败: {}", it.display(), e))?;
        }
        cmd.arg("--web-port")
            .arg(config_service::collect_port(&config).to_string())
            .arg("--cache-folder")
            .arg(&cache_dir)
            .arg("--data-dir")
            .arg(&data_dir)
            .arg("--max-download-thread-count")
            .arg(config.collect_thread_count.max(1).to_string())
            .arg("--yt-dlp")
            .arg(software_service::program(&software_service::YT_DLP))
            .current_dir(&dir);

        //客户端通过DairoMUSIC代理访问时只监听本机,采集服务的接口(如设置cookie)不对外开放
        if config.collect_public_mode != "direct" && supports_host(&program).await {
            cmd.arg("--host").arg("127.0.0.1");
        }

        //yt-dlp从PATH里查找ffmpeg、ffprobe、deno,把它们所在的目录加到PATH前面
        let mut paths = Vec::new();
        for it in [&software_service::FFMPEG, &software_service::FFPROBE, &software_service::DENO] {
            let program = software_service::program(it);
            if let Some(parent) = Path::new(&program).parent().filter(|it| !it.as_os_str().is_empty()) {
                if !paths.contains(&parent.to_path_buf()) {
                    paths.push(parent.to_path_buf());
                }
            }
        }
        if let Some(path) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&path));
        }
        if let Ok(path) = std::env::join_paths(paths) {
            cmd.env("PATH", path);
        }
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(it) = child.stdout.take() {
        tokio::spawn(read_output(sw, it));
    }
    if let Some(it) = child.stderr.take() {
        tokio::spawn(read_output(sw, it));
    }
    Ok(child)
}

/// 采集服务是否支持--host参数(老版本不支持,传了会启动失败)
async fn supports_host(program: &Path) -> bool {
    let output = Command::new(program)
        .arg("--help")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(Duration::from_secs(10), output).await {
        Ok(Ok(it)) => String::from_utf8_lossy(&it.stdout).contains("--host"),
        _ => false,
    }
}

/// 读取进程的输出,保存到日志
async fn read_output(sw: &'static Software, reader: impl AsyncRead + Unpin) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf);
                let line = line.trim_end();
                if !line.is_empty() {
                    log(sw, line);
                }
            }
        }
    }
}

/// 停止进程:先发送SIGINT让它正常退出,10秒内没有退出时强制结束
/// DFS没有处理退出信号,直接结束(数据库有事务保护,正在上传的文件客户端可以续传)
async fn kill(sw: &Software, child: &mut Child) {
    if sw == &DFS {
        let _ = child.kill().await;
        return;
    }
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGINT);
        }
        if tokio::time::timeout(Duration::from_secs(10), child.wait()).await.is_ok() {
            return;
        }
        log(sw, "10秒内没有退出,强制结束");
    }
    #[cfg(not(unix))]
    let _ = sw;
    let _ = child.kill().await;
}

/// DairoMUSIC上次没有正常退出时,内置服务可能还在运行(占着端口),先结束它
/// 只结束记录的进程ID对应的程序还是内置程序的进程,避免误杀进程ID被重新使用的其他程序
async fn kill_orphan(sw: &Software, program: &Path) {
    let pid_file = software_service::dir(sw).join("pid");
    let Some(pid) = std::fs::read_to_string(&pid_file).ok().and_then(|it| it.trim().parse::<u32>().ok()) else {
        return;
    };
    let _ = std::fs::remove_file(&pid_file);
    #[cfg(unix)]
    {
        let Ok(output) = Command::new("ps").args(["-p", &pid.to_string(), "-o", "command="]).output().await else {
            return;
        };
        let command = String::from_utf8_lossy(&output.stdout);
        if !command.trim().starts_with(&*program.to_string_lossy()) {
            return;
        }
        log(sw, format!("结束上次没有停止的进程: {}", pid));
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    #[cfg(not(unix))]
    let _ = (pid, program);
}
