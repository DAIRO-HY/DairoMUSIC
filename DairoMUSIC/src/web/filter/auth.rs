use crate::dao::{admin_dao, admin_menu_bar_dao, admin_token_dao};
use crate::util::{request_util, time_util};
use crate::web::biz_error::BizError;
use hyper::HeaderMap;
use hyperoute_core::{Next, Request, Response};
use hyperoute_macro::FILTER;
use std::time::Duration;

/// 当前登录的管理员,登录验证通过后放进请求扩展里,handler通过 Ext<LoginAdmin> 获取
#[derive(Debug, Clone)]
pub struct LoginAdmin {
    // 管理员ID
    pub id: i64,

    // 角色ID(从数据库获取,不信任请求里的参数)
    pub role_id: i64,
}

/// 后台接口需要的页面权限:接口路径前缀 -> 菜单里的页面链接
/// 角色有这个页面的菜单权限时才能调用对应的接口;不在这个列表里的接口只要登录就能调用
/// 前缀有包含关系时,长的写在前面
const API_PERMISSIONS: [(&str, &str); 16] = [
    ("/admin/api/swiper/", "/admin/swiper_list.html"),
    ("/admin/api/useful/", "/admin/useful_list.html"),
    ("/admin/api/favorite/", "/admin/favorite_list.html"),
    ("/admin/api/admin/", "/admin/admin_list.html"),
    ("/admin/api/role/", "/admin/admin_role_list.html"),
    ("/admin/api/menu/", "/admin/admin_menu_list.html"),
    ("/admin/api/user/", "/admin/user_list.html"),
    ("/admin/api/vip_code/", "/admin/vip_code_list.html"),
    ("/admin/api/music/", "/admin/music_list.html"),
    ("/admin/api/music_duplicate/", "/admin/music_duplicate_list.html"),
    ("/admin/api/system_config/", "/admin/system_config.html"),
    ("/admin/api/dfs_config/", "/admin/dfs_config.html"),
    ("/admin/api/search_config/", "/admin/search_config.html"),
    ("/admin/api/software/dfs/", "/admin/dfs_config.html"),
    ("/admin/api/software/search/", "/admin/search_config.html"),
    ("/admin/api/software/", "/admin/software.html"),
];

/// 验证后台是否已经登录,以及角色是否有接口的权限
#[FILTER(
    order = 10,
    include = ["/admin/api/**"],
    exclude = ["/admin/api/login/do_login", "/admin/api/install/**"]
)]
pub async fn auth_admin(mut req: Request, next: Next) -> Result<Response, BizError> {
    let Some(login) = find_login_admin(req.headers()).await? else {
        return Err(BizError::unauthorized());
    };
    let path = req.uri().path();
    if let Some((_, url)) = API_PERMISSIONS.iter().find(|(prefix, _)| path.starts_with(prefix)) {
        let mut ctx = lib_db::get_context();
        let count = admin_menu_bar_dao::count_role_url(&mut ctx, login.role_id, url.to_string()).await?;
        if count == 0 {
            return Err(BizError::forbidden());
        }
    }
    req.extensions_mut().insert(login);
    Ok(next.run(req).await)
}

/// 通过Cookie里的登录票据获取当前登录的管理员,没有登录、已过期或已被禁用时返回None
pub async fn find_login_admin(headers: &HeaderMap) -> Result<Option<LoginAdmin>, BizError> {
    let Some(token) = request_util::get_cookie(headers, request_util::ADMIN_TOKEN_COOKIE) else {
        return Ok(None);
    };
    if token.is_empty() {
        return Ok(None);
    }
    let mut ctx = lib_db::get_context();
    let Some(admin_token) = admin_token_dao::select_by_token(&mut ctx, token.to_string()).await? else {
        return Ok(None);
    };
    if admin_token.date < expire_time() {
        return Ok(None);
    }
    let Ok(admin) = admin_dao::select_one(&mut ctx, admin_token.adminId).await else {
        return Ok(None);
    };
    if admin.state != 1 {
        return Ok(None);
    }
    Ok(Some(LoginAdmin {
        id: admin.id,
        role_id: admin.roleId,
    }))
}

/// 早于这个时间的登录记录已经过期
fn expire_time() -> i64 {
    time_util::current_millis() - request_util::ADMIN_TOKEN_MAX_AGE * 1000
}

/// 每小时清理一次过期的登录记录
pub fn start_clean_task() {
    tokio::spawn(async {
        loop {
            let mut ctx = lib_db::get_context();
            if let Err(e) = admin_token_dao::delete_expired(&mut ctx, expire_time()).await {
                eprintln!("-->清理过期的登录记录失败: {}", e);
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
}
