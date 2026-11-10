//! 开发测试用的接口,只在启动参数 --mock-email 时可用,并且只允许本机访问

use crate::application;
use crate::service::email_service;
use crate::web::biz_error::BizError;
use hyperoute_core::RemoteAddr;
use hyperoute_macro::GET;
use serde::Serialize;

/// 模拟发送的验证码
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailCode {
    // 验证码类型 REGISTER:注册 RESET_PWD:重置密码 BIND_EMAIL:绑定邮箱
    pub r#type: String,

    // 验证码
    pub code: String,

    // 发送时间
    pub date: i64,
}

/// 获取邮箱最近一次发送的、还有效的验证码,没有时返回null
#[GET("/api/dev/email_code")]
pub async fn email_code(email: String, addr: RemoteAddr) -> Result<Option<EmailCode>, BizError> {
    //没有开启模拟发送邮件,或者不是本机访问时,当作接口不存在
    if !application::ARGS.mock_email || !addr.ip().is_loopback() {
        return Err(BizError::not_found());
    }
    let record = email_service::last_valid_code(email.trim()).await?;
    Ok(record.and_then(|it| {
        Some(EmailCode {
            code: it.code?,
            r#type: it.emailType,
            date: it.date,
        })
    }))
}
