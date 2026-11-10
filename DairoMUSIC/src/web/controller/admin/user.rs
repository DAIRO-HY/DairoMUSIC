//! 会员管理

use crate::dao::user_dao;
use crate::dao::user_dao::User;
use crate::service::dfs_service;
use crate::util::{pwd_util, time_util};
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::web::model::{non_empty, PageData, PageForm};
use crate::{biz_error, db};
use hyperoute_core::Query;
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use validator::Validate;

/// 会员信息(不包含密码)
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserItem {
    pub id: i64,
    pub email: Option<String>,
    pub nick: Option<String>,
    pub sex: i64,
    pub phone: Option<String>,
    pub logo: Option<String>,
    pub state: i64,
    pub last_login_time: i64,
    pub date: i64,
    pub temp_flag: i64,
    pub friend_flag: i64,
    pub test_flag: i64,
}

impl From<User> for UserItem {
    fn from(it: User) -> Self {
        Self {
            id: it.id,
            email: it.email,
            nick: it.nick,
            sex: it.sex,
            phone: it.phone,
            logo: dfs_service::file_url(it.logo.as_deref()),
            state: it.state,
            last_login_time: it.lastLoginTime,
            date: it.date,
            temp_flag: it.tempFlag,
            friend_flag: it.friendFlag,
            test_flag: it.testFlag,
        }
    }
}

/// 会员列表
#[GET("/admin/api/user/list")]
pub async fn list(page: Query<PageForm>, id: Option<i64>, email: Option<String>, state: Option<i64>) -> Result<PageData<UserItem>, BizError> {
    let page_in = page.to_page_in(&["id", "email", "state", "lastLoginTime", "date"], "id", "desc");
    let mut ctx = lib_db::get_context();
    let (total, list) = user_dao::page_list(&mut ctx, page_in, id, non_empty(email), state).await?;
    Ok(PageData::new(total, list.into_iter().map(UserItem::from).collect()))
}

/// 会员详情,id为0时返回新增用的默认值
#[GET("/admin/api/user/detail")]
pub async fn detail(id: i64) -> Result<UserItem, BizError> {
    if id == 0 {
        return Ok(UserItem {
            state: 1,
            ..Default::default()
        });
    }
    let mut ctx = lib_db::get_context();
    let Ok(user) = user_dao::select_one(&mut ctx, id).await else {
        return Err(biz_error!("会员不存在"));
    };
    Ok(user.into())
}

/// 添加或修改会员
#[POST("/admin/api/user/edit")]
pub async fn edit(form: form::UserEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let is_new = form.id == 0;
    if (is_new || !form.pwd.is_empty()) && form.pwd.len() < 6 {
        return Err(BizError::field("pwd", "密码长度必须在6到32位之间"));
    }
    if !(0..=2).contains(&form.state) {
        return Err(BizError::field("state", "状态不正确"));
    }
    let pwd = if form.pwd.is_empty() {
        None
    } else {
        Some(pwd_util::hash(&form.pwd).await)
    };
    let mut ctx = db::begin().await?;
    if user_dao::count_by_email(&mut ctx, form.email.clone(), form.id).await? > 0 {
        return Err(BizError::field("email", "该邮箱已经被使用"));
    }
    if is_new {
        let user = User {
            email: Some(form.email),
            pwd,
            state: form.state,
            date: time_util::current_millis(),
            friendFlag: form.friend_flag,
            ..Default::default()
        };
        user_dao::insert(&mut ctx, &user).await?;
    } else {
        if user_dao::select_one(&mut ctx, form.id).await.is_err() {
            return Err(biz_error!("会员不存在"));
        }
        user_dao::update_by_admin(&mut ctx, form.id, form.email, form.state, form.friend_flag).await?;
        if let Some(pwd) = pwd {
            user_dao::update_pwd(&mut ctx, form.id, pwd).await?;
        }
    }
    ctx.commit().await?;
    Ok(())
}
