//! 系统配置(后台"系统配置"页面设置,保存在system_config表)

use crate::dao::system_config_dao;
use serde::{Deserialize, Serialize};
use sqlx_context::DbContext;
use rand::distr::{Alphanumeric, SampleString};
use std::sync::RwLock;

/// 系统配置
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    // 是否开放注册(关闭时也不能创建临时账号)
    pub allow_register: bool,

    // 发送邮件的SMTP服务器,如 smtp.qq.com
    pub smtp_host: String,

    // SMTP端口,465为SSL,587为STARTTLS
    pub smtp_port: u16,

    // SMTP账号(发件邮箱)
    pub smtp_user: String,

    // SMTP密码(授权码)
    pub smtp_pwd: String,

    // 发件人名称
    pub smtp_from_name: String,

    // 是否启用歌曲采集(DairoMUSIC-Search)
    pub collect_enable: bool,

    // 采集服务的地址,如 http://127.0.0.1:8021
    pub collect_url: String,

    // 采集服务对外的地址,客户端直接播放采集平台的歌曲时使用;为空时和collect_url相同
    pub collect_public_url: String,

    // 是否采集视频(需要启用歌曲采集)
    pub video_enable: bool,

    // 视频编码器 空:自动选择(优先使用硬件编码) software:软件编码(libx264/libx265) videotoolbox:苹果硬件编码 nvenc:NVIDIA显卡 qsv:Intel核显
    pub video_encoder: String,

    // 视频转码使用的线程数,0:自动(CPU核数的一半)
    pub video_threads: i64,

    // 是否允许客户端把下载的视频保存到相册
    pub allow_save_album: bool,

    // 是否允许客户端下载歌曲(没有设置过时默认允许)
    #[serde(default = "default_true")]
    pub allow_download_music: bool,

    // 是否允许客户端下载视频(没有设置过时默认允许)
    #[serde(default = "default_true")]
    pub allow_download_video: bool,

    // 是否允许客户端把下载的歌曲文件发送到APP外面(隔空投送、其他APP等;没有设置过时默认允许),只在允许下载歌曲时有效
    #[serde(default = "default_true")]
    pub allow_send_music: bool,

    // 是否开启VIP功能(开启之后不是VIP的会员只能听标准音质的歌曲、看流畅版的视频,不能下载)
    pub vip_enable: bool,

    // 已经是VIP的会员再兑换时,VIP有效期最多可以累加到多少年(没有设置过时为2年)
    #[serde(default = "default_vip_max_years")]
    pub vip_max_years: i64,

    // 采集服务是否使用内置安装的DairoMUSIC-Search(随DairoMUSIC一起启动),否则使用collect_url
    #[serde(default = "default_true")]
    pub collect_builtin: bool,

    // 客户端访问采集服务的方式 proxy(默认,空也是):通过DairoMUSIC代理 direct:直接访问collect_public_url
    pub collect_public_mode: String,

    // 内置采集服务的端口,0时使用默认端口8021
    pub collect_port: u16,

    // 内置采集服务的数据目录,为空时使用 {数据目录}/search
    pub collect_data_dir: String,

    // 内置采集服务同时下载歌曲的数量,0时为1
    pub collect_thread_count: i64,

    // 是否使用内置安装的DairoDFS(随DairoMUSIC一起启动),否则使用dfs_url
    #[serde(default = "default_true")]
    pub dfs_builtin: bool,

    // 外部DairoDFS的接口地址,如 http://127.0.0.1:8031
    pub dfs_url: String,

    // 外部DairoDFS的API票据
    pub dfs_token: String,

    // 客户端访问DFS文件的方式 proxy(默认,空也是):通过DairoMUSIC代理 direct:直接访问dfs_public_url
    pub dfs_public_mode: String,

    // DFS对外的访问地址(direct时使用),为空时:外部DFS和dfs_url相同,内置DFS使用客户端访问DairoMUSIC的域名+DFS端口
    pub dfs_public_url: String,

    // 内置DFS的端口,0时使用默认端口8031
    pub dfs_port: u16,

    // 内置DFS的数据目录(数据库等),为空时使用 {数据目录}/dfs
    pub dfs_data_dir: String,

    // 内置DFS的文件存储目录,为空时使用 {DFS数据目录}/storage
    pub dfs_storage_dir: String,

    // 内置DFS的管理员用户名,为空时为admin
    pub dfs_admin_name: String,

    // 内置DFS的管理员密码,第一次初始化时自动生成
    pub dfs_admin_pwd: String,

    // 内置DFS自动生成的API票据
    pub dfs_builtin_token: String,

    // ffmpeg程序路径,为空时使用PATH里的ffmpeg(安装了内置的ffmpeg时优先使用内置的)
    pub ffmpeg_path: String,

    // ffprobe程序路径,为空时使用PATH里的ffprobe
    pub ffprobe_path: String,

    // yt-dlp程序路径,为空时使用PATH里的yt-dlp(只有内置采集服务使用)
    pub yt_dlp_path: String,

    // deno程序路径,为空时使用PATH里的deno(yt-dlp解析YouTube时使用)
    pub deno_path: String,

    // 服务器唯一标识,第一次启动时生成,以后不变(客户端用来区分不同服务器的缓存)
    pub server_id: String,
}

fn default_true() -> bool {
    true
}

fn default_vip_max_years() -> i64 {
    2
}

/// 内存里的配置
static CONFIG: RwLock<Option<Config>> = RwLock::new(None);

/// 启动时从数据库加载配置
pub async fn init() {
    let mut ctx = lib_db::get_context();
    let list = system_config_dao::select_all(&mut ctx).await.unwrap_or_default();
    let mut map = serde_json::Map::new();
    for it in list {
        //值都以JSON格式保存
        let value = serde_json::from_str(&it.value).unwrap_or(serde_json::Value::String(it.value));
        map.insert(it.name, value);
    }
    let mut config: Config = serde_json::from_value(serde_json::Value::Object(map)).unwrap_or_default();
    if config.smtp_port == 0 {
        config.smtp_port = 465;
    }
    if config.server_id.is_empty() {
        //第一次启动,生成服务器唯一标识
        config.server_id = Alphanumeric.sample_string(&mut rand::rng(), 32).to_lowercase();
        let value = serde_json::Value::String(config.server_id.clone()).to_string();
        if let Err(e) = system_config_dao::save(&mut ctx, "serverId".to_string(), value).await {
            eprintln!("-->保存服务器唯一标识失败: {}", e);
            std::process::exit(1);
        }
    }
    *CONFIG.write().unwrap() = Some(config);
}

/// 获取当前配置
pub fn get() -> Config {
    CONFIG.read().unwrap().clone().unwrap_or_default()
}

/// 保存配置(在事务里调用,提交后再调用apply)
pub async fn save(ctx: &mut DbContext, config: &Config) -> Result<(), sqlx::Error> {
    let serde_json::Value::Object(map) = serde_json::to_value(config).unwrap() else {
        return Ok(());
    };
    for (name, value) in map {
        system_config_dao::save(ctx, name, value.to_string()).await?;
    }
    Ok(())
}

/// 保存成功后更新内存里的配置
pub fn apply(config: Config) {
    *CONFIG.write().unwrap() = Some(config);
}

/// 修改一部分配置并保存(读取当前配置,修改后整体保存)
pub async fn update(f: impl FnOnce(&mut Config)) -> Result<Config, sqlx::Error> {
    //同一时间只有一个修改,避免两个修改同时读取后互相覆盖
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _lock = LOCK.lock().await;
    let mut config = get();
    f(&mut config);
    let mut ctx = crate::db::begin().await?;
    save(&mut ctx, &config).await?;
    ctx.commit().await?;
    apply(config.clone());
    Ok(config)
}

/// 数据目录的绝对路径(传给子进程的路径都用绝对路径)
pub fn data_dir() -> std::path::PathBuf {
    let dir = std::path::Path::new(&crate::application::ARGS.data_dir);
    std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf())
}

/// 内置DFS的端口
pub fn dfs_port(config: &Config) -> u16 {
    if config.dfs_port == 0 { 8031 } else { config.dfs_port }
}

/// 内置DFS的数据目录
pub fn dfs_data_dir(config: &Config) -> std::path::PathBuf {
    if config.dfs_data_dir.is_empty() {
        data_dir().join("dfs")
    } else {
        std::path::absolute(&config.dfs_data_dir).unwrap_or_else(|_| config.dfs_data_dir.clone().into())
    }
}

/// 内置DFS的文件存储目录
pub fn dfs_storage_dir(config: &Config) -> std::path::PathBuf {
    if config.dfs_storage_dir.is_empty() {
        dfs_data_dir(config).join("storage")
    } else {
        std::path::absolute(&config.dfs_storage_dir).unwrap_or_else(|_| config.dfs_storage_dir.clone().into())
    }
}

/// 内置DFS的管理员用户名
pub fn dfs_admin_name(config: &Config) -> String {
    if config.dfs_admin_name.is_empty() { "admin".to_string() } else { config.dfs_admin_name.clone() }
}

/// 内置采集服务的端口
pub fn collect_port(config: &Config) -> u16 {
    if config.collect_port == 0 { 8021 } else { config.collect_port }
}

/// 内置采集服务的数据目录
pub fn collect_data_dir(config: &Config) -> std::path::PathBuf {
    if config.collect_data_dir.is_empty() {
        data_dir().join("search")
    } else {
        std::path::absolute(&config.collect_data_dir).unwrap_or_else(|_| config.collect_data_dir.clone().into())
    }
}
