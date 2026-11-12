//! 采集服务(DairoMUSIC-Search)配置

use crate::biz_error;
use crate::service::software_service::SEARCH;
use crate::service::{collect_service, collect_task, config_service, process_service, video_task};
use crate::web::biz_error::BizError;
use crate::web::controller::admin::service_util::{check_dir, check_public_mode, check_url, parse_port};
use crate::web::form;
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use validator::Validate;

/// 采集服务配置
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchConfigVo {
    pub enable: bool,
    pub builtin: bool,
    pub url: String,
    pub public_mode: String,
    pub public_url: String,
    pub port: String,
    pub data_dir: String,
    pub thread_count: String,

    // 默认值(没有填写时使用)
    pub default_port: u16,
    pub default_data_dir: String,
}

/// 获取配置
#[GET("/admin/api/search_config/detail")]
pub async fn detail() -> SearchConfigVo {
    let config = config_service::get();
    let defaults = config_service::Config {
        collect_data_dir: String::new(),
        ..config.clone()
    };
    SearchConfigVo {
        enable: config.collect_enable,
        builtin: config.collect_builtin,
        url: config.collect_url.clone(),
        public_mode: if config.collect_public_mode.is_empty() { "proxy".to_string() } else { config.collect_public_mode.clone() },
        public_url: config.collect_public_url.clone(),
        port: if config.collect_port == 0 { String::new() } else { config.collect_port.to_string() },
        data_dir: config.collect_data_dir.clone(),
        thread_count: if config.collect_thread_count == 0 { String::new() } else { config.collect_thread_count.to_string() },
        default_port: 8021,
        default_data_dir: config_service::collect_data_dir(&defaults).to_string_lossy().to_string(),
    }
}

/// 保存配置,内置采集服务的配置有变化时重启
#[POST("/admin/api/search_config/save")]
pub async fn save(form: form::SearchConfig) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let enable = form.enable == 1;
    let builtin = form.builtin == 1;
    let url = check_url("url", &form.url)?;
    let public_url = check_url("publicUrl", &form.public_url)?;
    let public_mode = check_public_mode(&form.public_mode)?;
    let port = parse_port("port", &form.port)?;
    let data_dir = check_dir("dataDir", &form.data_dir)?;
    let thread_count = match form.thread_count.trim() {
        "" => 0,
        it => match it.parse::<i64>() {
            Ok(it) if (1..=16).contains(&it) => it,
            _ => return Err(BizError::field("threadCount", "必须是1到16之间的数字")),
        },
    };
    if enable && !builtin && url.is_empty() {
        return Err(BizError::field("url", "请填写采集服务的地址"));
    }
    if port != 0 && port == crate::application::ARGS.web_port {
        return Err(BizError::field("port", "不能和DairoMUSIC使用同一个端口"));
    }
    let old = config_service::get();
    let config = config_service::update(|it| {
        it.collect_enable = enable;
        it.collect_builtin = builtin;
        it.collect_url = url;
        it.collect_public_mode = public_mode;
        it.collect_public_url = public_url;
        it.collect_port = port;
        it.collect_data_dir = data_dir;
        it.collect_thread_count = thread_count;
    })
    .await?;
    let changed = old.collect_enable != config.collect_enable
        || old.collect_builtin != config.collect_builtin
        || old.collect_port != config.collect_port
        || old.collect_data_dir != config.collect_data_dir
        || old.collect_thread_count != config.collect_thread_count
        || old.collect_public_mode != config.collect_public_mode;
    if changed || !process_service::is_running(&SEARCH) {
        process_service::apply_config(&SEARCH).await;
    }

    //可能启用了采集,有等待处理的任务时开始处理
    collect_task::wake();
    video_task::wake();
    Ok(())
}

/// 测试连接(使用已保存的配置)
#[POST("/admin/api/search_config/test")]
pub async fn test() -> Result<String, BizError> {
    collect_service::test().await.map_err(|e| biz_error!(e))
}

/// 上传YouTube的cookie
#[POST("/admin/api/search_config/cookie")]
pub async fn cookie(cookie: String) -> Result<(), BizError> {
    if cookie.trim().is_empty() {
        return Err(BizError::field("cookie", "请填写cookie"));
    }
    collect_service::set_cookie(&cookie).await.map_err(|e| biz_error!(e))
}
