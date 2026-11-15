//! 系统配置

use crate::service::config_service::{self, Config};
use crate::service::email_service;
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::biz_error;
use hyperoute_macro::{GET, POST};
use validator::Validate;

/// 获取系统配置(不返回密码和票据)
#[GET("/admin/api/system_config/detail")]
pub async fn detail() -> Config {
    let mut config = config_service::get();
    config.smtp_pwd = String::new();
    config.dfs_token = String::new();
    config.dfs_admin_pwd = String::new();
    config.dfs_builtin_token = String::new();
    config
}

/// 保存系统配置
#[POST("/admin/api/system_config/save")]
pub async fn save(form: form::SystemConfig) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    if !["", "software", "videotoolbox", "nvenc", "qsv"].contains(&form.video_encoder.as_str()) {
        return Err(BizError::field("videoEncoder", "视频编码器不正确"));
    }

    //采集服务、DFS、ffmpeg等在各自的页面设置,这里只修改本页面的配置
    config_service::update(|it| {
        it.allow_register = form.allow_register == 1;
        it.smtp_host = form.smtp_host.trim().to_string();
        it.smtp_port = form.smtp_port;
        it.smtp_user = form.smtp_user.trim().to_string();

        //密码为空表示不修改
        if !form.smtp_pwd.is_empty() {
            it.smtp_pwd = form.smtp_pwd;
        }
        it.smtp_from_name = form.smtp_from_name.trim().to_string();
        it.video_enable = form.video_enable == 1;
        it.video_encoder = form.video_encoder;
        it.video_threads = form.video_threads;
        it.allow_save_album = form.allow_save_album == 1;
        it.allow_download_music = form.allow_download_music == 1;
        it.allow_download_video = form.allow_download_video == 1;
        it.allow_send_music = form.allow_send_music == 1;
        it.vip_enable = form.vip_enable == 1;
        it.vip_max_years = form.vip_max_years;
    })
    .await?;

    //可能启用了采集,有等待处理的任务时开始处理
    crate::service::collect_task::wake();
    crate::service::video_task::wake();
    Ok(())
}

/// 检查ffmpeg和ffprobe是否可以使用(使用已保存的配置)
#[POST("/admin/api/system_config/check_ffmpeg")]
pub async fn check_ffmpeg() -> Result<String, BizError> {
    let ffmpeg = crate::util::ffmpeg_util::version(false).await;
    let ffprobe = crate::util::ffmpeg_util::version(true).await;
    let text = |name: &str, it: Result<String, String>| match it {
        Ok(version) => format!("{}: {}", name, version),
        Err(e) => format!("{}: 不能使用({})", name, e),
    };
    let encoder = crate::util::video_util::encoders().await;
    Ok(format!(
        "{}\n{}\n视频编码器: H.264 {} / HEVC {}",
        text("ffmpeg", ffmpeg),
        text("ffprobe", ffprobe),
        encoder.h264,
        encoder.hevc
    ))
}

/// 发送测试邮件(使用已保存的配置)
#[POST("/admin/api/system_config/test_email")]
pub async fn test_email(email: String) -> Result<(), BizError> {
    let config = config_service::get();
    if config.smtp_host.is_empty() || config.smtp_user.is_empty() {
        return Err(biz_error!("请先保存邮件服务的配置"));
    }
    email_service::send(email.trim(), "DairoMUSIC测试邮件", "收到这封邮件说明邮件服务配置正确。")
        .await
        .map_err(|e| biz_error!(format!("发送失败: {}", e)))
}
