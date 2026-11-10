use crate::dao::admin_dao;
use crate::dao::admin_dao::Admin;
use crate::util::{pwd_util, time_util};
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::{biz_error, db};
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use validator::Validate;

/// 超级管理员的角色ID(init_data.sql里创建的)
const SUPER_ROLE_ID: i64 = 1;

/// 安装状态
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallStatus {
    // 是否已经创建了管理员
    pub is_init: bool,
}

/// 获取安装状态
#[GET("/admin/api/install/status")]
pub async fn status() -> Result<InstallStatus, BizError> {
    let mut ctx = lib_db::get_context();
    let count = admin_dao::count(&mut ctx).await?;
    Ok(InstallStatus { is_init: count > 0 })
}

/// 创建第一个管理员(超级管理员),已经有管理员时不允许调用
#[POST("/admin/api/install/create_admin")]
pub async fn create_admin(form: form::CreateAdmin) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let pwd = pwd_util::hash(&form.pwd).await;

    //在事务里检查,两个请求同时提交时只有一个能成功
    let mut ctx = db::begin().await?;
    if admin_dao::count(&mut ctx).await? > 0 {
        return Err(biz_error!("已经创建过管理员"));
    }
    let admin = Admin {
        id: 0,
        name: form.name,
        pwd,
        roleId: SUPER_ROLE_ID,
        realName: None,
        remark: None,
        state: 1,
        date: time_util::current_millis(),
        lastLoginTime: 0,
    };
    admin_dao::insert(&mut ctx, &admin).await?;
    ctx.commit().await?;
    Ok(())
}
