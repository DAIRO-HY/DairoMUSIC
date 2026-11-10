//! 系统配置

use crate::service::config_service::{self, Config};
use crate::service::email_service;
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::{biz_error, db};
use hyperoute_macro::{GET, POST};
use validator::Validate;

/// 获取系统配置(不返回SMTP密码)
#[GET("/admin/api/system_config/detail")]
pub async fn detail() -> Config {
    let mut config = config_service::get();
    config.smtp_pwd = String::new();
    config
}

/// 保存系统配置
#[POST("/admin/api/system_config/save")]
pub async fn save(form: form::SystemConfig) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let collect_url = form.collect_url.trim().to_string();
    let collect_public_url = form.collect_public_url.trim().to_string();
    if form.collect_enable == 1 && collect_url.is_empty() {
        return Err(BizError::field("collectUrl", "启用采集时必须设置采集服务的地址"));
    }
    for (field, url) in [("collectUrl", &collect_url), ("collectPublicUrl", &collect_public_url)] {
        if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(BizError::field(field, "地址必须以http://或https://开头"));
        }
    }
    if !["", "software", "videotoolbox", "nvenc", "qsv"].contains(&form.video_encoder.as_str()) {
        return Err(BizError::field("videoEncoder", "视频编码器不正确"));
    }
    let current = config_service::get();
    let config = Config {
        allow_register: form.allow_register == 1,
        smtp_host: form.smtp_host.trim().to_string(),
        smtp_port: form.smtp_port,
        smtp_user: form.smtp_user.trim().to_string(),

        //密码为空表示不修改
        smtp_pwd: if form.smtp_pwd.is_empty() { current.smtp_pwd } else { form.smtp_pwd },
        smtp_from_name: form.smtp_from_name.trim().to_string(),
        collect_enable: form.collect_enable == 1,
        collect_url,
        collect_public_url,
        video_enable: form.video_enable == 1,
        video_encoder: form.video_encoder,
        video_threads: form.video_threads,
        allow_save_album: form.allow_save_album == 1,
        allow_download_music: form.allow_download_music == 1,
        allow_download_video: form.allow_download_video == 1,
        ffmpeg_path: form.ffmpeg_path.trim().to_string(),
        ffprobe_path: form.ffprobe_path.trim().to_string(),
        server_id: current.server_id.clone(),
    };
    let mut ctx = db::begin().await?;
    config_service::save(&mut ctx, &config).await?;
    ctx.commit().await?;
    config_service::apply(config);

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
