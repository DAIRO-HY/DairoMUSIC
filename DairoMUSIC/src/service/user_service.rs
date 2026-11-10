//! 会员

use crate::dao::user_dao::User;
use crate::dao::user_favorite_dao::UserFavorite;
use crate::dao::user_login_dao::UserLogin;
use crate::dao::{user_dao, user_favorite_dao, user_login_dao};
use crate::util::time_util;
use rand::distr::{Alphanumeric, SampleString};
use sqlx_context::DbContext;

/// "我喜欢"歌单的名称
pub const LIKE_FAVORITE_NAME: &str = "我喜欢的音乐";

/// 添加会员,同时创建"我喜欢"歌单
/// - email 邮箱(临时会员为设备标识生成的邮箱)
/// - pwd 密码哈希,临时会员为None
/// - temp 是否临时会员
pub async fn register(ctx: &mut DbContext, email: String, pwd: Option<String>, temp: bool) -> Result<i64, sqlx::Error> {
    let now = time_util::current_millis();
    let user = User {
        email: Some(email),
        pwd,
        state: 1,
        date: now,
        lastLoginTime: now,
        tempFlag: temp as i64,
        ..Default::default()
    };
    let id = user_dao::insert(ctx, &user).await?;
    let favorite = UserFavorite {
        name: LIKE_FAVORITE_NAME.to_string(),
        likeFlag: 1,
        date: now,
        userId: id,
        ..Default::default()
    };
    user_favorite_dao::insert(ctx, &favorite).await?;
    Ok(id)
}

/// 添加登录记录,返回登录票据(同一台设备只保留一条)
pub async fn add_login(
    ctx: &mut DbContext,
    user_id: i64,
    device_id: String,
    client_flag: i64,
    client_version: i64,
    ip: String,
) -> Result<String, sqlx::Error> {
    user_login_dao::delete_by_device(ctx, user_id, device_id.clone()).await?;
    let token = Alphanumeric.sample_string(&mut rand::rng(), 32);
    let now = time_util::current_millis();
    let login = UserLogin {
        id: 0,
        token: token.clone(),
        userId: user_id,
        deviceId: device_id,
        clientFlag: client_flag,
        clientVersion: client_version,
        ip,
        date: now,
    };
    user_login_dao::insert(ctx, &login).await?;
    user_dao::update_last_login_time(ctx, user_id, now).await?;
    Ok(token)
}
