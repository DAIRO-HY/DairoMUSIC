//! 客户端:不需要登录的公共接口

use crate::dao::app_version_dao;
use crate::service::{collect_service, config_service};
use crate::web::biz_error::BizError;
use hyperoute_macro::GET;
use serde::Serialize;

/// 服务器的公开配置
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicConfig {
    // 服务器唯一标识(不会改变,客户端用来区分不同的服务器)
    pub server_id: String,

    // 服务器版本
    pub version: &'static str,

    // 是否开放注册(关闭时客户端隐藏注册和免注册使用)
    pub allow_register: bool,

    // 是否允许把下载的视频保存到相册
    pub allow_save_album: bool,

    // 是否允许下载歌曲(为false时客户端隐藏下载按钮)
    pub allow_download_music: bool,

    // 是否允许下载视频(为false时客户端隐藏视频的下载按钮)
    pub allow_download_video: bool,

    // 是否开启了视频采集(为false时还没有转码好的视频不会再处理,客户端不用轮询)
    pub video_enable: bool,
}

/// 新版本信息
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionVo {
    pub version_name: String,
    pub version_code: i64,

    // 安装方式 1:手动安装 2:弹框提示,后台下载 3:强制更新,后台下载
    pub install_type: i64,

    // 更新说明
    pub instruction: Option<String>,

    // 下载地址
    pub url: String,
}

/// 服务器的公开配置(客户端选择服务器时调用)
#[GET("/api/common/config")]
pub async fn config() -> PublicConfig {
    PublicConfig {
        server_id: config_service::get().server_id,
        version: crate::application::VERSION,
        allow_register: config_service::get().allow_register,
        allow_save_album: config_service::get().allow_save_album,
        allow_download_music: config_service::get().allow_download_music,
        allow_download_video: config_service::get().allow_download_video,
        video_enable: collect_service::is_video_enabled(),
    }
}

/// 检查更新,没有新版本时返回null
/// - client_flag 客户端标识 0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux
/// - version_code 当前版本号
#[GET("/api/app_version/check")]
pub async fn check_update(client_flag: i64, version_code: i64) -> Result<Option<AppVersionVo>, BizError> {
    let mut ctx = lib_db::get_context();
    let version = app_version_dao::check_update(&mut ctx, client_flag, version_code).await?;
    Ok(version.map(|it| AppVersionVo {
        version_name: it.versionName,
        version_code: it.versionCode,
        install_type: it.installType,
        instruction: it.instruction,
        url: it.url,
    }))
}
