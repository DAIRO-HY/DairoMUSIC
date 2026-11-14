//! 客户端接口的登录验证

use crate::dao::{user_dao, user_login_dao};
use crate::util::time_util;
use crate::web::biz_error::BizError;
use hyper::HeaderMap;
use hyperoute_core::{Next, Request, Response};
use hyperoute_macro::FILTER;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// 当前登录的会员,登录验证通过后放进请求扩展里,handler通过 Ext<LoginUser> 获取
#[derive(Debug, Clone)]
pub struct LoginUser {
    // 会员ID
    pub id: i64,

    // 登录票据
    pub token: String,

    // 是否临时会员
    pub is_temp: bool,

    // 是否信任会员
    pub is_friend: bool,

    // 是否测试用户(应用市场审核用,只能看到测试歌曲)
    pub is_test: bool,

    // 现在是否VIP
    pub is_vip: bool,
}

/// 最后在线时间的更新间隔(毫秒)
const ONLINE_UPDATE_INTERVAL: i64 = 3600 * 1000;

/// 最后一次更新在线时间: 会员ID -> 时间
static ONLINE_TIMES: LazyLock<Mutex<HashMap<i64, i64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// 验证客户端是否已经登录
#[FILTER(
    order = 10,
    include = ["/api/**"],
    exclude = [
        "/api/user/login",
        "/api/user/temp_login",
        "/api/user/register",
        "/api/user/reset_pwd",
        "/api/email/send_code",
        "/api/common/**",
        "/api/app_version/**",
        "/api/public/**",
        "/api/dev/**"
    ]
)]
pub async fn auth_user(mut req: Request, next: Next) -> Result<Response, BizError> {
    let login = find_login_user(req.headers()).await?;
    req.extensions_mut().insert(login);
    Ok(next.run(req).await)
}

/// 获取请求头 Authorization: Bearer <token> 里的登录票据
pub fn get_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|it| it.to_str().ok())
        .and_then(|it| it.strip_prefix("Bearer "))
        .map(|it| it.trim())
        .filter(|it| !it.is_empty())
}

/// 获取当前登录的会员
/// 没有登录、票据无效、已注销时返回401;被禁用时返回403
pub async fn find_login_user(headers: &HeaderMap) -> Result<LoginUser, BizError> {
    let Some(token) = get_token(headers) else {
        return Err(BizError::unauthorized());
    };
    let mut ctx = lib_db::get_context();
    let Some(login) = user_login_dao::select_by_token(&mut ctx, token.to_string()).await? else {
        return Err(BizError::unauthorized());
    };
    let Ok(user) = user_dao::select_one(&mut ctx, login.userId).await else {
        return Err(BizError::unauthorized());
    };
    match user.state {
        1 => {}
        0 => return Err(BizError::forbidden_msg("该账号已被禁用")),
        _ => return Err(BizError::unauthorized()),
    }
    update_online_time(user.id).await;
    Ok(LoginUser {
        id: user.id,
        token: login.token,
        is_temp: user.tempFlag == 1,
        is_friend: user.friendFlag == 1,
        is_test: user.testFlag == 1,
        is_vip: crate::service::vip_service::is_vip(&user),
    })
}

/// 更新最后在线时间(每小时最多更新一次,避免每个请求都写数据库)
async fn update_online_time(user_id: i64) {
    let now = time_util::current_millis();
    {
        let mut map = ONLINE_TIMES.lock().unwrap();
        if map.get(&user_id).is_some_and(|it| now - it < ONLINE_UPDATE_INTERVAL) {
            return;
        }
        map.insert(user_id, now);
    }
    let mut ctx = lib_db::get_context();
    if let Err(e) = user_dao::update_last_login_time(&mut ctx, user_id, now).await {
        eprintln!("-->更新最后在线时间失败: {}", e);
    }
}
