//! DairoDFS配置

use crate::service::config_service;
use crate::service::software_service::DFS;
use crate::service::{dfs_service, process_service};
use crate::web::biz_error::BizError;
use crate::web::controller::admin::service_util::{check_dir, check_public_mode, check_url, parse_port};
use crate::web::form;
use crate::biz_error;
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use validator::Validate;

/// DairoDFS配置(不返回密码和票据)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DfsConfigVo {
    pub builtin: bool,
    pub url: String,

    // 是否已经设置了外部DFS的API票据
    pub has_token: bool,
    pub public_mode: String,
    pub public_url: String,
    pub port: String,
    pub data_dir: String,
    pub storage_dir: String,
    pub admin_name: String,

    // 是否已经有管理员密码
    pub has_admin_pwd: bool,

    // 内置DFS是否已经生成了API票据(已经初始化)
    pub has_builtin_token: bool,

    // 默认值(没有填写时使用)
    pub default_port: u16,
    pub default_data_dir: String,
    pub default_storage_dir: String,
}

/// 获取配置
#[GET("/admin/api/dfs_config/detail")]
pub async fn detail() -> DfsConfigVo {
    let config = config_service::get();
    let defaults = config_service::Config {
        dfs_data_dir: String::new(),
        dfs_storage_dir: String::new(),
        ..config.clone()
    };
    DfsConfigVo {
        builtin: config.dfs_builtin,
        url: config.dfs_url.clone(),
        has_token: !config.dfs_token.is_empty(),
        public_mode: if config.dfs_public_mode.is_empty() { "proxy".to_string() } else { config.dfs_public_mode.clone() },
        public_url: config.dfs_public_url.clone(),
        port: if config.dfs_port == 0 { String::new() } else { config.dfs_port.to_string() },
        data_dir: config.dfs_data_dir.clone(),
        storage_dir: config.dfs_storage_dir.clone(),
        admin_name: config.dfs_admin_name.clone(),
        has_admin_pwd: !config.dfs_admin_pwd.is_empty(),
        has_builtin_token: !config.dfs_builtin_token.is_empty(),
        default_port: 8031,
        default_data_dir: config_service::dfs_data_dir(&defaults).to_string_lossy().to_string(),
        default_storage_dir: config_service::dfs_storage_dir(&config).to_string_lossy().to_string(),
    }
}

/// 保存配置,内置DFS的配置有变化时重启DFS
#[POST("/admin/api/dfs_config/save")]
pub async fn save(form: form::DfsConfig) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let builtin = form.builtin == 1;
    let url = check_url("url", &form.url)?;
    let public_url = check_url("publicUrl", &form.public_url)?;
    let public_mode = check_public_mode(&form.public_mode)?;
    let port = parse_port("port", &form.port)?;
    let data_dir = check_dir("dataDir", &form.data_dir)?;
    let storage_dir = check_dir("storageDir", &form.storage_dir)?;
    if !builtin && url.is_empty() {
        return Err(BizError::field("url", "请填写DFS的地址"));
    }
    if port != 0 && port == crate::application::ARGS.web_port {
        return Err(BizError::field("port", "不能和DairoMUSIC使用同一个端口"));
    }
    let old = config_service::get();
    if !builtin && form.token.is_empty() && old.dfs_token.is_empty() {
        return Err(BizError::field("token", "请填写DFS的API票据"));
    }
    let admin_name = form.admin_name.trim().to_string();
    let config = config_service::update(|it| {
        it.dfs_builtin = builtin;
        it.dfs_url = url;
        if !form.token.is_empty() {
            it.dfs_token = form.token.trim().to_string();
        }
        it.dfs_public_mode = public_mode;
        it.dfs_public_url = public_url;
        it.dfs_port = port;
        it.dfs_data_dir = data_dir;
        it.dfs_storage_dir = storage_dir;

        //管理员账号改了之后,重新登录生成API票据
        if admin_name != it.dfs_admin_name || !form.admin_pwd.is_empty() {
            it.dfs_builtin_token = String::new();
        }
        it.dfs_admin_name = admin_name;
        if !form.admin_pwd.is_empty() {
            it.dfs_admin_pwd = form.admin_pwd;
        }
    })
    .await?;

    //内置DFS的启动参数或开关变了时重启(或停止)
    let changed = old.dfs_builtin != config.dfs_builtin
        || old.dfs_port != config.dfs_port
        || old.dfs_data_dir != config.dfs_data_dir
        || old.dfs_storage_dir != config.dfs_storage_dir
        || old.dfs_builtin_token != config.dfs_builtin_token;
    if changed || !process_service::is_running(&DFS) {
        process_service::apply_config(&DFS).await;
    }
    dfs_service::reset();
    Ok(())
}

/// 测试连接(使用已保存的配置)
#[POST("/admin/api/dfs_config/test")]
pub async fn test() -> Result<String, BizError> {
    dfs_service::test().await.map_err(|e| biz_error!(e.to_string()))
}

/// 查看内置DFS的管理员密码(登录DFS的网页时使用)
#[POST("/admin/api/dfs_config/admin_pwd")]
pub async fn admin_pwd() -> Result<String, BizError> {
    let config = config_service::get();
    if config.dfs_admin_pwd.is_empty() {
        return Err(biz_error!("还没有管理员密码,DFS第一次启动时自动生成"));
    }
    Ok(config.dfs_admin_pwd)
}

/// 重新初始化:重新登录内置DFS,生成API票据
#[POST("/admin/api/dfs_config/reinit")]
pub async fn reinit() -> Result<(), BizError> {
    if !process_service::is_running(&DFS) {
        return Err(biz_error!("内置DFS没有运行"));
    }
    config_service::update(|it| it.dfs_builtin_token = String::new()).await?;
    tokio::spawn(dfs_service::init_builtin());
    Ok(())
}
