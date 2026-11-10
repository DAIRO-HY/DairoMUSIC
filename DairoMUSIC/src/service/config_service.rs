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

    // 是否启用歌曲采集(DairoMusicSearch)
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

    // ffmpeg程序路径,为空时使用PATH里的ffmpeg
    pub ffmpeg_path: String,

    // ffprobe程序路径,为空时使用PATH里的ffprobe
    pub ffprobe_path: String,

    // 服务器唯一标识,第一次启动时生成,以后不变(客户端用来区分不同服务器的缓存)
    pub server_id: String,
}

fn default_true() -> bool {
    true
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
