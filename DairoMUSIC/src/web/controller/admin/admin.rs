//! 管理员管理

use crate::dao::admin_dao::{Admin, PageListTemp};
use crate::dao::{admin_dao, admin_role_dao, admin_token_dao};
use crate::util::{pwd_util, time_util};
use crate::web::biz_error::BizError;
use crate::web::filter::auth::LoginAdmin;
use crate::web::form;
use crate::web::model::{non_empty, OptionItem, PageData, PageForm};
use crate::{biz_error, db};
use hyperoute_core::{Ext, Query};
use hyperoute_macro::{DELETE, GET, POST};
use serde::Serialize;
use validator::Validate;

/// 管理员详情(不包含密码)
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AdminDetail {
    pub id: i64,
    pub name: String,
    pub real_name: Option<String>,
    pub remark: Option<String>,
    pub role_id: i64,
    pub state: i64,
    pub date: i64,
    pub last_login_time: i64,
}

/// 管理员列表
#[GET("/admin/api/admin/list")]
pub async fn list(page: Query<PageForm>, name: Option<String>, role_id: Option<i64>) -> Result<PageData<PageListTemp>, BizError> {
    let page_in = page.to_page_in(&["id", "name", "roleId", "state", "date", "lastLoginTime"], "id", "asc");
    let mut ctx = lib_db::get_context();
    let (total, list) = admin_dao::page_list(&mut ctx, page_in, non_empty(name), role_id).await?;
    Ok(PageData::new(total, list))
}

/// 可以选择的角色
#[GET("/admin/api/admin/roles")]
pub async fn roles() -> Result<Vec<OptionItem>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = admin_role_dao::select_list(&mut ctx)
        .await?
        .into_iter()
        .map(|it| OptionItem { value: it.id, text: it.name })
        .collect();
    Ok(list)
}

/// 管理员详情,id为0时返回新增用的默认值
#[GET("/admin/api/admin/detail")]
pub async fn detail(id: i64) -> Result<AdminDetail, BizError> {
    if id == 0 {
        return Ok(AdminDetail {
            state: 1,
            ..Default::default()
        });
    }
    let mut ctx = lib_db::get_context();
    let Ok(admin) = admin_dao::select_one(&mut ctx, id).await else {
        return Err(biz_error!("管理员不存在"));
    };
    Ok(AdminDetail {
        id: admin.id,
        name: admin.name,
        real_name: admin.realName,
        remark: admin.remark,
        role_id: admin.roleId,
        state: admin.state,
        date: admin.date,
        last_login_time: admin.lastLoginTime,
    })
}

/// 添加或修改管理员
#[POST("/admin/api/admin/edit")]
pub async fn edit(login: Ext<LoginAdmin>, form: form::AdminEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    if form.id == 0 && form.pwd.len() < 6 {
        return Err(BizError::field("pwd", "密码长度必须在6到32位之间"));
    }
    if form.id != 0 && !form.pwd.is_empty() && form.pwd.len() < 6 {
        return Err(BizError::field("pwd", "密码长度必须在6到32位之间"));
    }
    if form.id == login.id && form.state != 1 {
        return Err(BizError::field("state", "不能禁用自己的账号"));
    }
    let pwd = if form.pwd.is_empty() {
        None
    } else {
        Some(pwd_util::hash(&form.pwd).await)
    };
    let mut ctx = db::begin().await?;
    if admin_role_dao::select_one(&mut ctx, form.role_id).await.is_err() {
        return Err(BizError::field("roleId", "请选择角色"));
    }
    if admin_dao::count_by_name(&mut ctx, form.name.clone(), form.id).await? > 0 {
        return Err(BizError::field("name", "该登录名已经存在"));
    }
    let real_name = non_empty(Some(form.real_name));
    let remark = non_empty(Some(form.remark));
    if form.id == 0 {
        let admin = Admin {
            id: 0,
            name: form.name,
            pwd: pwd.unwrap_or_default(),
            roleId: form.role_id,
            realName: real_name,
            remark,
            state: form.state,
            date: time_util::current_millis(),
            lastLoginTime: 0,
        };
        admin_dao::insert(&mut ctx, &admin).await?;
    } else {
        if admin_dao::select_one(&mut ctx, form.id).await.is_err() {
            return Err(biz_error!("管理员不存在"));
        }
        let param = admin_dao::UpdateInfoParam {
            id: form.id,
            name: form.name,
            real_name,
            remark,
            role_id: form.role_id,
            state: form.state,
        };
        admin_dao::update_info(&mut ctx, param).await?;
        if let Some(pwd) = pwd {
            admin_dao::update_pwd(&mut ctx, form.id, pwd).await?;
        }

        //修改了密码或者禁用时,已经登录的设备需要重新登录(修改自己的时候除外)
        if form.id != login.id && (!form.pwd.is_empty() || form.state != 1) {
            admin_token_dao::delete_by_admin_id(&mut ctx, form.id).await?;
        }
    }
    ctx.commit().await?;
    Ok(())
}

/// 删除管理员
#[DELETE("/admin/api/admin/delete")]
pub async fn delete(login: Ext<LoginAdmin>, id: i64) -> Result<(), BizError> {
    if id == login.id {
        return Err(biz_error!("不能删除自己的账号"));
    }
    let mut ctx = db::begin().await?;
    admin_dao::delete(&mut ctx, id).await?;
    admin_token_dao::delete_by_admin_id(&mut ctx, id).await?;
    ctx.commit().await?;
    Ok(())
}
