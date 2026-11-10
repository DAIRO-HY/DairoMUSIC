//! 导航菜单管理

use crate::dao::admin_menu_bar_dao::{AdminMenuBar, SelectListTemp};
use crate::dao::{admin_menu_bar_dao, admin_permission_dao};
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::web::model::{non_empty, OptionItem};
use crate::{biz_error, db};
use hyperoute_macro::{DELETE, GET, POST};
use validator::Validate;

/// 所有菜单
#[GET("/admin/api/menu/list")]
pub async fn list() -> Result<Vec<SelectListTemp>, BizError> {
    let mut ctx = lib_db::get_context();
    Ok(admin_menu_bar_dao::select_list(&mut ctx).await?)
}

/// 可以选择的上级菜单
#[GET("/admin/api/menu/parents")]
pub async fn parents() -> Result<Vec<OptionItem>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = admin_menu_bar_dao::select_tops(&mut ctx)
        .await?
        .into_iter()
        .map(|it| OptionItem { value: it.id, text: it.name })
        .collect();
    Ok(list)
}

/// 菜单详情,id为0时返回新增用的默认值
#[GET("/admin/api/menu/detail")]
pub async fn detail(id: i64) -> Result<AdminMenuBar, BizError> {
    if id == 0 {
        return Ok(AdminMenuBar {
            state: 1,
            ..Default::default()
        });
    }
    let mut ctx = lib_db::get_context();
    admin_menu_bar_dao::select_one(&mut ctx, id)
        .await
        .map_err(|_| biz_error!("菜单不存在"))
}

/// 添加或修改菜单
#[POST("/admin/api/menu/edit")]
pub async fn edit(form: form::MenuEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    if form.id != 0 && form.id == form.parent_id {
        return Err(BizError::field("parentId", "上级菜单不能是自己"));
    }
    let mut ctx = db::begin().await?;
    if form.parent_id != 0 {
        //只支持两级菜单
        let parent = admin_menu_bar_dao::select_one(&mut ctx, form.parent_id).await;
        if parent.is_err() || parent.is_ok_and(|it| it.parentId != 0) {
            return Err(BizError::field("parentId", "上级菜单必须是一级菜单"));
        }
        if form.id != 0 && admin_menu_bar_dao::count_children(&mut ctx, form.id).await? > 0 {
            return Err(BizError::field("parentId", "这个菜单下面还有子菜单,不能设为子菜单"));
        }
    }
    if admin_menu_bar_dao::count_by_name(&mut ctx, form.name.clone(), form.parent_id, form.id).await? > 0 {
        return Err(BizError::field("name", "同一级下已经有这个名称的菜单"));
    }
    let menu = AdminMenuBar {
        id: form.id,
        name: form.name,
        ico: non_empty(Some(form.ico)),
        url: non_empty(Some(form.url)),
        parentId: form.parent_id,
        state: form.state,
        orderNum: form.order_num,
    };
    if form.id == 0 {
        admin_menu_bar_dao::insert(&mut ctx, &menu).await?;
    } else {
        admin_menu_bar_dao::update(&mut ctx, &menu).await?;
    }
    ctx.commit().await?;
    Ok(())
}

/// 删除菜单
#[DELETE("/admin/api/menu/delete")]
pub async fn delete(id: i64) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    if admin_menu_bar_dao::count_children(&mut ctx, id).await? > 0 {
        return Err(biz_error!("请先删除这个菜单下的子菜单"));
    }
    admin_menu_bar_dao::delete(&mut ctx, id).await?;
    admin_permission_dao::delete_by_menu(&mut ctx, id).await?;
    ctx.commit().await?;
    Ok(())
}
