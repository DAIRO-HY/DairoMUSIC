use crate::dao::admin_token_dao::AdminToken;
use crate::dao::{admin_dao, admin_token_dao};
use crate::util::{pwd_util, request_util, time_util};
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::{biz_error, db};
use hyper::HeaderMap;
use hyperoute_core::{Raw, RemoteAddr};
use hyperoute_macro::{DELETE, POST};
use rand::distr::{Alphanumeric, SampleString};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use validator::Validate;

/// 统计登录失败次数的时间范围(毫秒)
const FAIL_WINDOW: i64 = 10 * 60 * 1000;

/// 同一个IP对同一个登录名允许连续失败的次数
const FAIL_LIMIT_IP: usize = 5;

/// 同一个登录名(不管从哪个IP)允许连续失败的次数,防止伪造IP绕过限制
const FAIL_LIMIT_NAME: usize = 20;

/// 登录失败记录: 登录名 -> [(IP, 失败时间)]
static LOGIN_FAILS: LazyLock<Mutex<HashMap<String, Vec<(String, i64)>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 后台登录
#[POST("/admin/api/login/do_login")]
pub async fn do_login(form: form::AdminLogin, headers: &HeaderMap, addr: RemoteAddr) -> Result<Raw, BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let ip = request_util::get_ip(headers, &addr);
    check_fail_limit(&form.name, &ip)?;

    let mut ctx = db::begin().await?;
    let admin = admin_dao::select_by_name(&mut ctx, form.name.clone()).await?;

    //用户不存在时也做一次哈希验证,响应时间一样,不能通过时间差判断用户名是否存在
    let hash = admin.as_ref().map(|it| it.pwd.as_str()).unwrap_or(DUMMY_HASH.as_str());
    let is_ok = pwd_util::verify(&form.pwd, hash).await;
    let Some(admin) = admin.filter(|_| is_ok) else {
        add_fail(&form.name, &ip);
        return Err(biz_error!("登录名或密码错误"));
    };
    if admin.state != 1 {
        return Err(biz_error!("该账号已被禁用"));
    }
    clear_fail(&form.name);

    let token = Alphanumeric.sample_string(&mut rand::rng(), 32);
    let now = time_util::current_millis();
    let admin_token = AdminToken {
        id: 0,
        token: token.clone(),
        adminId: admin.id,
        ip,
        date: now,
    };
    admin_token_dao::insert(&mut ctx, &admin_token).await?;
    admin_dao::update_last_login_time(&mut ctx, admin.id, now).await?;
    ctx.commit().await?;
    Ok(Raw::new("").header("set-cookie", request_util::admin_token_cookie(&token)))
}

/// 退出登录
#[DELETE("/admin/api/login/logout")]
pub async fn logout(headers: &HeaderMap) -> Result<Raw, BizError> {
    if let Some(token) = request_util::get_cookie(headers, request_util::ADMIN_TOKEN_COOKIE) {
        let mut ctx = lib_db::get_context();
        admin_token_dao::delete_by_token(&mut ctx, token.to_string()).await?;
    }
    Ok(Raw::new("").header("set-cookie", request_util::clear_admin_token_cookie()))
}

/// 用户不存在时用来验证的哈希(随机密码的哈希,不会和任何输入匹配)
static DUMMY_HASH: LazyLock<String> =
    LazyLock::new(|| pwd_util::hash_md5(&Alphanumeric.sample_string(&mut rand::rng(), 32)));

/// 检查登录失败次数是否超过限制
pub fn check_fail_limit(name: &str, ip: &str) -> Result<(), BizError> {
    let mut map = LOGIN_FAILS.lock().unwrap();
    let Some(list) = map.get_mut(name) else {
        return Ok(());
    };
    let start = time_util::current_millis() - FAIL_WINDOW;
    list.retain(|(_, time)| *time > start);
    let ip_count = list.iter().filter(|(it, _)| it == ip).count();
    if ip_count >= FAIL_LIMIT_IP || list.len() >= FAIL_LIMIT_NAME {
        return Err(biz_error!("登录失败次数过多,请10分钟后再试"));
    }
    Ok(())
}

/// 记录一次登录失败
pub fn add_fail(name: &str, ip: &str) {
    let mut map = LOGIN_FAILS.lock().unwrap();

    //记录太多时清理过期的,防止用随机登录名把内存撑满
    if map.len() > 10000 {
        let start = time_util::current_millis() - FAIL_WINDOW;
        map.retain(|_, list| list.iter().any(|(_, time)| *time > start));
    }
    map.entry(name.to_string())
        .or_default()
        .push((ip.to_string(), time_util::current_millis()));
}

/// 登录成功后清除失败记录
pub fn clear_fail(name: &str) {
    LOGIN_FAILS.lock().unwrap().remove(name);
}
