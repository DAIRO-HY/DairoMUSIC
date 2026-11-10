//! 角色和权限管理

use crate::dao::admin_role_dao::AdminRole;
use crate::dao::{admin_dao, admin_menu_bar_dao, admin_permission_dao, admin_role_dao};
use crate::web::biz_error::BizError;
use crate::web::filter::auth::LoginAdmin;
use crate::web::form;
use crate::web::model::non_empty;
use crate::{biz_error, db};
use hyperoute_core::Ext;
use hyperoute_macro::{DELETE, GET, POST};
use serde::Serialize;
use std::collections::HashSet;
use validator::Validate;

/// 菜单权限
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuPermission {
    pub id: i64,
    pub name: String,
    pub parent_id: i64,
    pub state: i64,

    // 是否有权限
    pub checked: bool,
}

/// 所有角色
#[GET("/admin/api/role/list")]
pub async fn list() -> Result<Vec<AdminRole>, BizError> {
    let mut ctx = lib_db::get_context();
    Ok(admin_role_dao::select_list(&mut ctx).await?)
}

/// 角色详情,id为0时返回新增用的默认值
#[GET("/admin/api/role/detail")]
pub async fn detail(id: i64) -> Result<AdminRole, BizError> {
    if id == 0 {
        return Ok(AdminRole::default());
    }
    let mut ctx = lib_db::get_context();
    admin_role_dao::select_one(&mut ctx, id)
        .await
        .map_err(|_| biz_error!("角色不存在"))
}

/// 添加或修改角色
#[POST("/admin/api/role/edit")]
pub async fn edit(form: form::RoleEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let mut ctx = db::begin().await?;
    if admin_role_dao::count_by_name(&mut ctx, form.name.clone(), form.id).await? > 0 {
        return Err(BizError::field("name", "该角色名已经存在"));
    }
    let role = AdminRole {
        id: form.id,
        name: form.name,
        remark: non_empty(Some(form.remark)),
    };
    if form.id == 0 {
        admin_role_dao::insert(&mut ctx, &role).await?;
    } else {
        admin_role_dao::update(&mut ctx, &role).await?;
    }
    ctx.commit().await?;
    Ok(())
}

/// 删除角色,还有管理员在使用时不允许删除
#[DELETE("/admin/api/role/delete")]
pub async fn delete(login: Ext<LoginAdmin>, id: i64) -> Result<(), BizError> {
    if id == login.role_id {
        return Err(biz_error!("不能删除自己所属的角色"));
    }
    let mut ctx = db::begin().await?;
    if admin_dao::count_by_role(&mut ctx, id).await? > 0 {
        return Err(biz_error!("还有管理员属于这个角色,请先修改这些管理员的角色"));
    }
    admin_role_dao::delete(&mut ctx, id).await?;
    admin_permission_dao::delete_by_role(&mut ctx, id).await?;
    ctx.commit().await?;
    Ok(())
}

/// 获取角色的菜单权限
#[GET("/admin/api/role/permission")]
pub async fn permission(role_id: i64) -> Result<Vec<MenuPermission>, BizError> {
    let mut ctx = lib_db::get_context();
    let ids: HashSet<i64> = admin_permission_dao::select_menu_ids(&mut ctx, role_id)
        .await?
        .into_iter()
        .collect();
    let list = admin_menu_bar_dao::select_list(&mut ctx)
        .await?
        .into_iter()
        .map(|it| MenuPermission {
            checked: ids.contains(&it.id),
            id: it.id,
            name: it.name,
            parent_id: it.parentId,
            state: it.state,
        })
        .collect();
    Ok(list)
}

/// 保存角色的菜单权限
#[POST("/admin/api/role/save_permission")]
pub async fn save_permission(login: Ext<LoginAdmin>, form: form::Permission) -> Result<(), BizError> {
    let mut menu_ids = Vec::new();
    for it in form.menu_ids.split(',').filter(|it| !it.is_empty()) {
        let Ok(id) = it.parse::<i64>() else {
            return Err(biz_error!("菜单ID格式错误"));
        };
        menu_ids.push(id);
    }
    let mut ctx = db::begin().await?;
    if admin_role_dao::select_one(&mut ctx, form.role_id).await.is_err() {
        return Err(biz_error!("角色不存在"));
    }
    admin_permission_dao::delete_by_role(&mut ctx, form.role_id).await?;
    for menu_id in &menu_ids {
        let permission = admin_permission_dao::AdminPermission {
            roleId: form.role_id,
            menuId: *menu_id,
        };
        admin_permission_dao::insert(&mut ctx, &permission).await?;
    }

    //防止把自己的角色管理权限去掉后再也改不回来
    if form.role_id == login.role_id
        && admin_menu_bar_dao::count_role_url(&mut ctx, form.role_id, ROLE_PAGE.to_string()).await? == 0
    {
        return Err(biz_error!("不能去掉自己所属角色的“角色列表”权限"));
    }
    ctx.commit().await?;
    Ok(())
}

/// 角色管理页面
pub const ROLE_PAGE: &str = "/admin/admin_role_list.html";
