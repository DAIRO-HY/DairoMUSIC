//! 依赖软件的安装(DairoDFS、DairoMUSIC-Search、ffmpeg、ffprobe、yt-dlp、deno)

use crate::biz_error;
use crate::service::process_service::{self, ServiceInfo};
use crate::service::software_service::{self, InstallState, Recommend, Software};
use crate::service::config_service;
use crate::web::biz_error::BizError;
use hyperoute_core::BodyStream;
use hyperoute_macro::{GET, POST};
use serde::Serialize;

/// 软件信息
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareVo {
    pub key: &'static str,
    pub name: &'static str,
    pub is_service: bool,

    // 是否安装了内置的程序
    pub installed: bool,

    // 内置程序的安装路径
    pub install_path: String,

    // 当前使用的程序(内置的、填写的路径或PATH里的)
    pub program: String,

    // 填写的外部程序路径(只有ffmpeg等工具)
    pub external_path: String,

    // 当前使用的程序的版本号,不能运行时为空
    pub version: String,

    // 不能运行的原因
    pub version_error: String,

    // 安装状态和日志
    pub install: InstallState,

    // 服务的运行状态(只有内置服务)
    pub service: Option<ServiceInfo>,
}

/// 获取软件
fn get(key: &str) -> Result<&'static Software, BizError> {
    software_service::find(key).ok_or_else(BizError::not_found)
}

/// 获取服务
fn get_service(key: &str) -> Result<&'static Software, BizError> {
    let sw = get(key)?;
    if !sw.is_service {
        return Err(BizError::not_found());
    }
    Ok(sw)
}

/// 软件信息
#[GET("/admin/api/software/{key}/info")]
pub async fn info(key: String) -> Result<SoftwareVo, BizError> {
    let sw = get(&key)?;
    let installed = software_service::is_installed(sw);
    let install = software_service::state(sw);

    //服务没有安装内置程序时不检查版本(外部服务不在本机)
    let version = if sw.is_service && !installed {
        Ok(String::new())
    } else if install.running {
        Ok(String::new())
    } else {
        software_service::version(sw).await
    };
    Ok(SoftwareVo {
        key: sw.key,
        name: sw.name,
        is_service: sw.is_service,
        installed,
        install_path: software_service::install_path(sw).to_string_lossy().to_string(),
        program: software_service::program(sw),
        external_path: software_service::external_path(sw),
        version: version.clone().unwrap_or_default(),
        version_error: version.err().unwrap_or_default(),
        install,
        service: if sw.is_service { Some(process_service::info(sw)) } else { None },
    })
}

/// 通过URL下载安装(后台执行,通过info查看进度)
#[POST("/admin/api/software/{key}/install_url")]
pub async fn install_url(key: String, url: String) -> Result<(), BizError> {
    let sw = get(&key)?;
    software_service::install_by_url(sw, url.trim().to_string()).map_err(|e| biz_error!(e))
}

/// 上传安装包,请求体直接是文件内容
/// - name 文件名
#[POST("/admin/api/software/{key}/upload")]
pub async fn upload(key: String, name: String, body: BodyStream) -> Result<(), BizError> {
    let sw = get(&key)?;
    software_service::install_by_upload(sw, name, body).await.map_err(|e| biz_error!(e))
}

/// 卸载内置的程序
#[POST("/admin/api/software/{key}/uninstall")]
pub async fn uninstall(key: String) -> Result<(), BizError> {
    let sw = get(&key)?;
    software_service::uninstall(sw).await.map_err(|e| biz_error!(e))
}

/// 当前系统推荐的下载地址
#[GET("/admin/api/software/{key}/recommend")]
pub async fn recommend(key: String) -> Result<Recommend, BizError> {
    let sw = get(&key)?;
    software_service::recommend(sw).await.map_err(|e| biz_error!(e))
}

/// 重新检查版本号
#[POST("/admin/api/software/{key}/refresh")]
pub async fn refresh(key: String) -> Result<(), BizError> {
    get(&key)?;
    software_service::clear_version_cache();
    Ok(())
}

/// 设置外部程序的路径(没有安装内置程序时使用)
#[POST("/admin/api/software/{key}/path")]
pub async fn path(key: String, path: String) -> Result<(), BizError> {
    let sw = get(&key)?;
    let path = path.trim().to_string();
    if path.len() > 512 {
        return Err(BizError::field("path", "长度不能超过512位"));
    }
    config_service::update(|it| match sw.key {
        "ffmpeg" => it.ffmpeg_path = path,
        "ffprobe" => it.ffprobe_path = path,
        "yt-dlp" => it.yt_dlp_path = path,
        "deno" => it.deno_path = path,
        _ => {}
    })
    .await?;
    software_service::clear_version_cache();
    Ok(())
}

/// 启动服务
#[POST("/admin/api/software/{key}/start")]
pub async fn start(key: String) -> Result<(), BizError> {
    let sw = get_service(&key)?;
    if !software_service::is_installed(sw) {
        return Err(biz_error!(format!("还没有安装{}", sw.name)));
    }
    process_service::start(sw);
    Ok(())
}

/// 停止服务
#[POST("/admin/api/software/{key}/stop")]
pub async fn stop(key: String) -> Result<(), BizError> {
    let sw = get_service(&key)?;
    process_service::stop(sw).await;
    Ok(())
}

/// 重启服务
#[POST("/admin/api/software/{key}/restart")]
pub async fn restart(key: String) -> Result<(), BizError> {
    let sw = get_service(&key)?;
    if !software_service::is_installed(sw) {
        return Err(biz_error!(format!("还没有安装{}", sw.name)));
    }
    process_service::restart(sw).await;
    Ok(())
}

/// 服务最近的日志
#[GET("/admin/api/software/{key}/logs")]
pub async fn logs(key: String) -> Result<Vec<String>, BizError> {
    let sw = get_service(&key)?;
    Ok(process_service::logs(sw))
}
