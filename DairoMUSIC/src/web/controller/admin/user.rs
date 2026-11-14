//! 会员管理

use crate::dao::user_dao;
use crate::dao::user_dao::User;
use crate::service::{dfs_service, user_service, vip_service};
use crate::util::{pwd_util, time_util};
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::web::model::{non_empty, PageData, PageForm};
use crate::{biz_error, db};
use hyperoute_core::Query;
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use validator::{Validate, ValidateEmail};

/// 会员信息(不包含密码)
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserItem {
    pub id: i64,
    pub email: Option<String>,

    // 用户名
    pub name: Option<String>,
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

    // VIP到期时间(毫秒),0:不是VIP
    pub vip_time: i64,

    // 是否永久VIP
    pub vip_forever: i64,

    // 现在是否VIP
    pub is_vip: bool,
}

impl From<User> for UserItem {
    fn from(it: User) -> Self {
        Self {
            is_vip: vip_service::is_vip(&it),
            vip_time: it.vipTime,
            vip_forever: it.vipForever,
            id: it.id,
            email: it.email,
            name: it.name,
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
pub async fn list(
    page: Query<PageForm>,
    id: Option<i64>,
    email: Option<String>,
    name: Option<String>,
    state: Option<i64>,
) -> Result<PageData<UserItem>, BizError> {
    let page_in = page.to_page_in(&["id", "email", "name", "state", "lastLoginTime", "date", "vipTime"], "id", "desc");
    let mut ctx = lib_db::get_context();
    let name = non_empty(name).map(|it| it.to_lowercase());
    let (total, list) = user_dao::page_list(&mut ctx, page_in, id, non_empty(email), name, state).await?;
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

    //邮箱和用户名至少填写一个
    let email = non_empty(Some(form.email));
    if email.as_ref().is_some_and(|it| !it.validate_email()) {
        return Err(BizError::field("email", "邮箱格式不正确"));
    }
    let name = match non_empty(Some(form.name)) {
        Some(it) => match user_service::check_name(&it) {
            Ok(it) => Some(it),
            Err(e) => return Err(BizError::field("name", e)),
        },
        None => None,
    };
    if email.is_none() && name.is_none() {
        return Err(BizError::field("email", "邮箱和用户名至少填写一个"));
    }
    let is_new = form.id == 0;
    if (is_new || !form.pwd.is_empty()) && form.pwd.len() < 6 {
        return Err(BizError::field("pwd", "密码长度必须在6到32位之间"));
    }
    if !(0..=2).contains(&form.state) {
        return Err(BizError::field("state", "状态不正确"));
    }
    let vip_forever = (form.vip_forever == 1) as i64;
    let vip_time = if form.vip_date.trim().is_empty() {
        0
    } else {
        let Some(vip_time) = time_util::date_end_millis(&form.vip_date) else {
            return Err(BizError::field("vipDate", "日期格式不正确"));
        };
        vip_time
    };
    let pwd = if form.pwd.is_empty() {
        None
    } else {
        Some(pwd_util::hash(&form.pwd).await)
    };
    let mut ctx = db::begin().await?;
    if let Some(email) = &email
        && user_dao::count_by_email(&mut ctx, email.clone(), form.id).await? > 0
    {
        return Err(BizError::field("email", "该邮箱已经被使用"));
    }
    if let Some(name) = &name
        && user_dao::count_by_name(&mut ctx, name.clone(), form.id).await? > 0
    {
        return Err(BizError::field("name", "该用户名已经被使用"));
    }
    if is_new {
        let now = time_util::current_millis();
        let user = User {
            email,
            name,
            pwd,
            state: form.state,
            date: now,
            friendFlag: form.friend_flag,
            vipTime: vip_time,
            vipForever: vip_forever,
            ..Default::default()
        };
        let user_id = user_dao::insert(&mut ctx, &user).await?;

        //每个会员都有一个"我喜欢"歌单
        user_service::add_like_favorite(&mut ctx, user_id, now).await?;
    } else {
        let Ok(user) = user_dao::select_one(&mut ctx, form.id).await else {
            return Err(biz_error!("会员不存在"));
        };
        let param = user_dao::UpdateByAdminParam {
            id: form.id,
            email,
            name,
            state: form.state,
            friend_flag: form.friend_flag,
        };
        user_dao::update_by_admin(&mut ctx, param).await?;

        //页面上只能选择日期;日期没有修改时保留原来的到期时间(兑换的到期时间精确到毫秒)
        let vip_time = if user.vipTime != 0 && time_util::date_text(user.vipTime) == form.vip_date.trim() { user.vipTime } else { vip_time };
        user_dao::update_vip(&mut ctx, form.id, vip_time, vip_forever).await?;
        if let Some(pwd) = pwd {
            user_dao::update_pwd(&mut ctx, form.id, pwd).await?;
        }
    }
    ctx.commit().await?;
    Ok(())
}
