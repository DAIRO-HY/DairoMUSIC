//! 邮件验证码

use crate::dao::email_dao;
use crate::dao::email_dao::Email;
use crate::service::config_service;
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::{biz_error, db};
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use rand::RngExt;
use sqlx_context::DbContext;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

/// 验证码类型:注册
pub const TYPE_REGISTER: &str = "REGISTER";

/// 验证码类型:重置密码
pub const TYPE_RESET_PWD: &str = "RESET_PWD";

/// 验证码类型:绑定邮箱
pub const TYPE_BIND_EMAIL: &str = "BIND_EMAIL";

/// 验证码有效期(毫秒)
const CODE_EXPIRE: i64 = 10 * 60 * 1000;

/// 同一个邮箱发送验证码的间隔(毫秒)
const SEND_INTERVAL: i64 = 60 * 1000;

/// 一小时内最多发送的邮件数量,防止被用来群发邮件
const HOURLY_LIMIT: i64 = 200;

/// 同一个验证码允许输错的次数,超过后验证码作废
const MAX_WRONG: u32 = 5;

/// 验证码输错的次数: 验证码记录ID -> 次数
static WRONG_COUNTS: LazyLock<Mutex<HashMap<i64, u32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// 发送验证码
/// - email_type 验证码类型 TYPE_xxx
pub async fn send_code(email: &str, email_type: &str) -> Result<(), BizError> {
    let config = config_service::get();
    let is_mock = crate::application::ARGS.mock_email;
    if !is_mock && (config.smtp_host.is_empty() || config.smtp_user.is_empty()) {
        return Err(biz_error!("服务器还没有配置邮件服务,请联系管理员"));
    }
    let now = time_util::current_millis();
    let code = format!("{:06}", rand::rng().random_range(0..1_000_000));
    let subject = match email_type {
        TYPE_REGISTER => format!("注册验证码:{}", code),
        TYPE_RESET_PWD => format!("重置密码验证码:{}", code),
        _ => format!("绑定邮箱验证码:{}", code),
    };
    let content = format!("您的验证码是:{},10分钟内有效。如果不是您本人操作,请忽略这封邮件。", code);

    //检查发送频率和保存记录在同一个事务里,同时请求时不会重复发送
    let mut ctx = db::begin().await?;
    //模拟发送时不限制发送间隔,自动测试会连续获取验证码
    if !is_mock && email_dao::count_since(&mut ctx, email.to_string(), now - SEND_INTERVAL).await? > 0 {
        return Err(biz_error!("发送太频繁,请1分钟后再试"));
    }
    if email_dao::count_all_since(&mut ctx, now - 3600 * 1000).await? >= HOURLY_LIMIT {
        return Err(biz_error!("服务器繁忙,请稍后再试"));
    }
    let record = Email {
        id: 0,
        fromEmail: config.smtp_user.clone(),
        email: email.to_string(),
        subject: Some(subject.clone()),
        content: content.clone(),
        emailType: email_type.to_string(),
        code: Some(code),
        state: 0,
        error: None,
        sendTime: None,
        date: now,
    };
    let id = email_dao::insert(&mut ctx, &record).await?;
    ctx.commit().await?;
    if is_mock {
        println!("-->[模拟发送邮件] {} {} 验证码:{}", email, email_type, record.code.as_deref().unwrap_or_default());
        let mut ctx = lib_db::get_context();
        email_dao::update_state(&mut ctx, id, 1, Some("模拟发送,没有发送邮件".to_string()), time_util::current_millis()).await?;
        return Ok(());
    }

    let result = send(email, &subject, &content).await;
    let mut ctx = lib_db::get_context();
    match result {
        Ok(_) => {
            email_dao::update_state(&mut ctx, id, 1, None, time_util::current_millis()).await?;
            Ok(())
        }
        Err(e) => {
            eprintln!("-->发送邮件失败: {}", e);
            email_dao::update_state(&mut ctx, id, 2, Some(e.clone()), time_util::current_millis()).await?;
            Err(biz_error!("邮件发送失败,请稍后再试"))
        }
    }
}

/// 获取邮箱最近一次发送的、还有效的验证码(模拟发送邮件时,开发测试用)
pub async fn last_valid_code(email: &str) -> Result<Option<Email>, BizError> {
    let mut ctx = lib_db::get_context();
    let since = time_util::current_millis() - CODE_EXPIRE;
    Ok(email_dao::select_last_valid_code(&mut ctx, email.to_string(), since).await?)
}

/// 验证验证码,正确时作废这个验证码(只能用一次)
/// - field 验证失败时显示错误的表单字段名
pub async fn verify_code(ctx: &mut DbContext, email: &str, email_type: &str, code: &str) -> Result<(), BizError> {
    let Some(record) = email_dao::select_last_code(ctx, email.to_string(), email_type.to_string()).await? else {
        return Err(BizError::field("code", "验证码错误"));
    };
    if record.date < time_util::current_millis() - CODE_EXPIRE {
        return Err(BizError::field("code", "验证码已过期,请重新获取"));
    }
    if record.code.as_deref() != Some(code) {
        let count = {
            let mut map = WRONG_COUNTS.lock().unwrap();
            let count = map.entry(record.id).or_insert(0);
            *count += 1;
            *count
        };
        if count >= MAX_WRONG {
            //输错太多次,作废这个验证码,防止暴力猜测
            email_dao::clear_code(&mut lib_db::get_context(), record.id).await?;
            WRONG_COUNTS.lock().unwrap().remove(&record.id);
            return Err(BizError::field("code", "验证码错误次数过多,请重新获取"));
        }
        return Err(BizError::field("code", "验证码错误"));
    }
    email_dao::clear_code(ctx, record.id).await?;
    WRONG_COUNTS.lock().unwrap().remove(&record.id);
    Ok(())
}

/// 发送邮件
pub async fn send(to: &str, subject: &str, content: &str) -> Result<(), String> {
    let config = config_service::get();
    let from_name = if config.smtp_from_name.is_empty() {
        "DairoMUSIC".to_string()
    } else {
        config.smtp_from_name.clone()
    };
    let from: Mailbox = format!("{} <{}>", from_name, config.smtp_user)
        .parse()
        .map_err(|e| format!("发件邮箱格式错误: {}", e))?;
    let to: Mailbox = to.parse().map_err(|e| format!("收件邮箱格式错误: {}", e))?;
    let message = Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(content.to_string())
        .map_err(|e| e.to_string())?;
    let builder = if config.smtp_port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)
    }
    .map_err(|e| e.to_string())?;
    let mailer = builder
        .port(config.smtp_port)
        .credentials(Credentials::new(config.smtp_user.clone(), config.smtp_pwd.clone()))
        .timeout(Some(Duration::from_secs(20)))
        .build();
    mailer.send(message).await.map_err(|e| e.to_string())?;
    Ok(())
}
