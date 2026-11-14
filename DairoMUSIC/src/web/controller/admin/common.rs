use crate::dao::{admin_dao, admin_menu_bar_dao, admin_token_dao, music_complaint_dao, music_correct_dao};
use crate::util::{pwd_util, request_util};
use crate::web::biz_error::BizError;
use crate::web::filter::auth::LoginAdmin;
use crate::web::form;
use crate::db;
use hyperoute_core::{Ext, Raw};
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use validator::Validate;

/// 左侧菜单
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Menu {
    pub id: i64,

    // 菜单名称
    pub name: String,

    // 图标
    pub ico: Option<String>,

    // 页面链接
    pub url: Option<String>,

    // 子菜单
    pub children: Vec<Menu>,
}

/// 当前登录的管理员信息
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub name: String,
    pub real_name: Option<String>,
}

/// 获取当前角色可以使用的菜单(两级)
#[GET("/admin/api/common/menu")]
pub async fn menu(login: Ext<LoginAdmin>) -> Result<Vec<Menu>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = admin_menu_bar_dao::select_actives_by_role(&mut ctx, login.role_id).await?;
    let (tops, subs): (Vec<_>, Vec<_>) = list.into_iter().partition(|it| it.parentId == 0);
    let menus = tops
        .into_iter()
        .map(|top| Menu {
            children: subs
                .iter()
                .filter(|it| it.parentId == top.id)
                .map(|it| Menu {
                    id: it.id,
                    name: it.name.clone(),
                    ico: it.ico.clone(),
                    url: it.url.clone(),
                    children: Vec::new(),
                })
                .collect(),
            id: top.id,
            name: top.name,
            ico: top.ico,
            url: top.url,
        })
        //没有链接也没有子菜单的分组不显示
        .filter(|it| it.url.is_some() || !it.children.is_empty())
        .collect();
    Ok(menus)
}

/// 待处理的事项数量,没有对应页面权限的不返回
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    // 还没有处理的纠错数量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correct: Option<i64>,

    // 还没有处理的投诉数量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complaint: Option<i64>,
}

/// 首页显示的待处理事项:还没有处理的纠错、投诉数量
#[GET("/admin/api/common/todo")]
pub async fn todo(login: Ext<LoginAdmin>) -> Result<Todo, BizError> {
    let mut ctx = lib_db::get_context();
    let mut todo = Todo { correct: None, complaint: None };
    if admin_menu_bar_dao::count_role_url(&mut ctx, login.role_id, "/admin/music_correct_list.html".to_string()).await? > 0 {
        todo.correct = Some(music_correct_dao::count_pending(&mut ctx).await?);
    }
    if admin_menu_bar_dao::count_role_url(&mut ctx, login.role_id, "/admin/music_complaint_list.html".to_string()).await? > 0 {
        todo.complaint = Some(music_complaint_dao::count_pending(&mut ctx).await?);
    }
    Ok(todo)
}

/// 获取当前登录的管理员信息
#[GET("/admin/api/common/info")]
pub async fn info(login: Ext<LoginAdmin>) -> Result<Info, BizError> {
    let mut ctx = lib_db::get_context();
    let admin = admin_dao::select_one(&mut ctx, login.id).await?;
    Ok(Info {
        name: admin.name,
        real_name: admin.realName,
    })
}

/// 修改自己的密码,修改后其他设备上的登录全部失效,当前设备需要重新登录
#[POST("/admin/api/common/modify_pwd")]
pub async fn modify_pwd(login: Ext<LoginAdmin>, form: form::ModifyPwd) -> Result<Raw, BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let mut ctx = db::begin().await?;
    let admin = admin_dao::select_one(&mut ctx, login.id).await?;
    if !pwd_util::verify(&form.old_pwd, &admin.pwd).await {
        return Err(BizError::field("oldPwd", "当前密码不正确"));
    }
    let pwd = pwd_util::hash(&form.pwd).await;
    admin_dao::update_pwd(&mut ctx, login.id, pwd).await?;
    admin_token_dao::delete_by_admin_id(&mut ctx, login.id).await?;
    ctx.commit().await?;
    Ok(Raw::new("").header("set-cookie", request_util::clear_admin_token_cookie()))
}
