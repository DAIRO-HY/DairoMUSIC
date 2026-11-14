//! 客户端:账号

use crate::dao::{user_dao, user_login_dao};
use crate::service::vip_service::{self, VipVo};
use crate::service::{config_service, dfs_service, email_service, image_service, user_service};
use crate::util::{ffmpeg_util, image_util, pwd_util, request_util, time_util};
use crate::web::biz_error::BizError;
use crate::web::filter::user_auth::{self, LoginUser};
use crate::web::form;
use crate::web::model::non_empty;
use crate::{biz_error, db};
use hyper::HeaderMap;
use hyperoute_core::{Ext, RemoteAddr};
use hyperoute_macro::{DELETE, GET, POST};
use rand::distr::{Alphanumeric, SampleString};
use serde::Serialize;
use validator::Validate;

/// 登录结果
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResult {
    // 登录票据,之后的请求放在请求头 Authorization: Bearer <token>
    pub token: String,

    // 会员信息
    pub user: UserInfo,
}

/// 会员信息
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: i64,

    // 邮箱,临时会员、只有用户名的会员为空
    pub email: Option<String>,

    // 用户名(后台设置,可以代替邮箱登录),没有时为空
    pub name: Option<String>,

    // 昵称
    pub nick: Option<String>,

    // 头像地址
    pub logo: Option<String>,

    // 是否临时会员(还没有绑定邮箱)
    pub is_temp: bool,

    // 是否信任会员(纠错直接生效,可以审核歌曲)
    pub is_friend: bool,

    // VIP状态
    pub vip: VipVo,
}

/// 获取会员信息
async fn user_info(id: i64) -> Result<UserInfo, BizError> {
    let mut ctx = lib_db::get_context();
    let user = user_dao::select_one(&mut ctx, id).await?;
    Ok(UserInfo {
        vip: vip_service::vo(&user),
        id: user.id,
        name: user.name,
        nick: user.nick,
        logo: dfs_service::file_url(user.logo.as_deref()),
        is_temp: user.tempFlag == 1,
        is_friend: user.friendFlag == 1,
        email: if user.tempFlag == 1 { None } else { user.email },
    })
}

/// 免注册使用:每台设备对应一个临时会员,同一台设备再次调用时找回同一个临时会员
#[POST("/api/user/temp_login")]
pub async fn temp_login(form: form::Device, headers: &HeaderMap, addr: RemoteAddr) -> Result<LoginResult, BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }

    //临时会员的邮箱用设备标识生成,不是真实的邮箱
    let temp_email = format!("{}@temp.device", form.device_id);
    let mut ctx = db::begin().await?;
    let user_id = match user_dao::select_by_email(&mut ctx, temp_email.clone()).await? {
        Some(user) => {
            if user.state != 1 {
                return Err(BizError::forbidden_msg("该账号已被禁用"));
            }
            user.id
        }
        None => {
            if !config_service::get().allow_register {
                return Err(biz_error!("暂未开放注册"));
            }
            user_service::register(&mut ctx, temp_email, None, true).await?
        }
    };
    let ip = request_util::get_ip(headers, &addr);
    let token = user_service::add_login(&mut ctx, user_id, form.device_id, form.client_flag, form.client_version, ip).await?;
    ctx.commit().await?;
    Ok(LoginResult {
        token,
        user: user_info(user_id).await?,
    })
}

/// 用邮箱或者用户名登录
#[POST("/api/user/login")]
pub async fn login(form: form::UserLogin, headers: &HeaderMap, addr: RemoteAddr) -> Result<LoginResult, BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let ip = request_util::get_ip(headers, &addr);
    //失败次数按小写的账号统计
    let account = form.account.trim().to_lowercase();
    crate::web::controller::admin::login::check_fail_limit(&account, &ip)?;
    let mut ctx = db::begin().await?;
    let user = user_service::select_by_account(&mut ctx, &account)
        .await?
        .filter(|it| it.tempFlag == 0 && it.state != 2);
    let hash = user.as_ref().and_then(|it| it.pwd.clone()).unwrap_or_default();
    let is_ok = !hash.is_empty() && pwd_util::verify(&form.pwd, &hash).await;
    let Some(user) = user.filter(|_| is_ok) else {
        crate::web::controller::admin::login::add_fail(&account, &ip);
        return Err(biz_error!("账号或密码不正确"));
    };
    if user.state != 1 {
        return Err(BizError::forbidden_msg("该账号已被禁用"));
    }
    crate::web::controller::admin::login::clear_fail(&account);
    let token = user_service::add_login(&mut ctx, user.id, form.device_id, form.client_flag, form.client_version, ip).await?;
    ctx.commit().await?;
    Ok(LoginResult {
        token,
        user: user_info(user.id).await?,
    })
}

/// 注册
#[POST("/api/user/register")]
pub async fn register(form: form::UserRegister, headers: &HeaderMap, addr: RemoteAddr) -> Result<LoginResult, BizError> {
    if !config_service::get().allow_register {
        return Err(biz_error!("暂未开放注册"));
    }
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let pwd = pwd_util::hash(&form.pwd).await;
    let mut ctx = db::begin().await?;
    email_service::verify_code(&mut ctx, &form.email, email_service::TYPE_REGISTER, &form.code).await?;
    if user_dao::count_by_email(&mut ctx, form.email.clone(), 0).await? > 0 {
        return Err(BizError::field("email", "该邮箱已经注册,忘记密码时请使用重置密码"));
    }
    let user_id = user_service::register(&mut ctx, form.email, Some(pwd), false).await?;
    let ip = request_util::get_ip(headers, &addr);
    let token = user_service::add_login(&mut ctx, user_id, form.device_id, form.client_flag, form.client_version, ip).await?;
    ctx.commit().await?;
    Ok(LoginResult {
        token,
        user: user_info(user_id).await?,
    })
}

/// 通过邮箱验证码重置密码,重置后所有设备需要重新登录
#[POST("/api/user/reset_pwd")]
pub async fn reset_pwd(form: form::ResetPwd) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let pwd = pwd_util::hash(&form.pwd).await;
    let mut ctx = db::begin().await?;
    email_service::verify_code(&mut ctx, &form.email, email_service::TYPE_RESET_PWD, &form.code).await?;
    let user = user_dao::select_by_email(&mut ctx, form.email).await?.filter(|it| it.tempFlag == 0);
    let Some(user) = user else {
        return Err(BizError::field("email", "该邮箱还没有注册"));
    };
    user_dao::update_pwd(&mut ctx, user.id, pwd).await?;
    user_login_dao::delete_by_user(&mut ctx, user.id).await?;
    ctx.commit().await?;
    Ok(())
}

/// 绑定邮箱
/// - 临时会员:绑定后成为正式会员,歌单等数据都保留,pwd是新的登录密码
/// - 只有用户名的会员(后台创建的):pwd是当前的登录密码,用来确认身份,密码不变
#[POST("/api/user/bind_email")]
pub async fn bind_email(login: Ext<LoginUser>, form: form::BindEmail) -> Result<UserInfo, BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let mut ctx = db::begin().await?;
    if !login.is_temp {
        let user = user_dao::select_one(&mut ctx, login.id).await?;
        if user.email.is_some() {
            return Err(biz_error!("已经绑定过邮箱"));
        }
        let hash = user.pwd.unwrap_or_default();
        if hash.is_empty() || !pwd_util::verify(&form.pwd, &hash).await {
            return Err(BizError::field("pwd", "密码不正确"));
        }
        email_service::verify_code(&mut ctx, &form.email, email_service::TYPE_BIND_EMAIL, &form.code).await?;
        if user_dao::count_by_email(&mut ctx, form.email.clone(), login.id).await? > 0 {
            return Err(BizError::field("email", "该邮箱已经被其他账号使用"));
        }
        user_dao::set_email(&mut ctx, login.id, form.email).await?;
        ctx.commit().await?;
        return user_info(login.id).await;
    }
    let pwd = pwd_util::hash(&form.pwd).await;
    email_service::verify_code(&mut ctx, &form.email, email_service::TYPE_BIND_EMAIL, &form.code).await?;
    if user_dao::count_by_email(&mut ctx, form.email.clone(), login.id).await? > 0 {
        return Err(BizError::field("email", "该邮箱已经被其他账号使用"));
    }
    user_dao::bind_email(&mut ctx, login.id, form.email, pwd).await?;
    ctx.commit().await?;
    user_info(login.id).await
}

/// 获取会员信息
#[GET("/api/user/info")]
pub async fn info(login: Ext<LoginUser>) -> Result<UserInfo, BizError> {
    user_info(login.id).await
}

/// 修改昵称
#[POST("/api/user/edit")]
pub async fn edit(login: Ext<LoginUser>, form: form::UserEditInfo) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let mut ctx = lib_db::get_context();
    user_dao::update_nick(&mut ctx, login.id, non_empty(Some(form.nick))).await?;
    Ok(())
}

/// 头像上传
pub struct AvatarUpload {
    // 图片内容
    pub file: Vec<u8>,
}

/// 上传头像,返回头像地址
#[POST("/api/user/avatar")]
pub async fn avatar(login: Ext<LoginUser>, form: AvatarUpload) -> Result<Option<String>, BizError> {
    let path = upload_image(form.file).await?;
    let mut ctx = lib_db::get_context();
    let old = user_dao::select_one(&mut ctx, login.id).await?.logo;
    user_dao::update_logo(&mut ctx, login.id, Some(path.clone())).await?;
    image_service::remove_replaced(old, Some(&path));
    Ok(dfs_service::file_url(Some(&path)))
}

/// 上传图片(头像、歌单封面、歌曲封面)到DairoDFS,返回存储路径
/// 不限制上传的图片大小,上传后居中裁切成正方形并缩小到800x800,转换成JPG
pub async fn upload_image(data: Vec<u8>) -> Result<String, BizError> {
    if image_util::image_ext(&data).is_none() {
        return Err(biz_error!("只支持JPG、PNG、WEBP格式的图片"));
    }
    let data = match ffmpeg_util::crop_image_data(data, ffmpeg_util::LOGO_SIZE, ffmpeg_util::LOGO_SIZE).await {
        Ok(it) => it,
        Err(e) => {
            eprintln!("-->图片处理失败: {}", e);
            return Err(biz_error!("图片处理失败,请换一张图片再试"));
        }
    };
    let name = Alphanumeric.sample_string(&mut rand::rng(), 16);
    let path = format!("/image/logo/{}/{}.jpg", time_util::current_year(), name);
    if let Err(e) = dfs_service::upload(data, &path, "").await {
        eprintln!("-->图片上传失败: {}", e);
        return Err(biz_error!("图片上传失败,请稍后再试"));
    }
    Ok(path)
}

/// 修改密码,修改后其他设备需要重新登录,当前设备继续使用
#[POST("/api/user/modify_pwd")]
pub async fn modify_pwd(login: Ext<LoginUser>, form: form::ModifyPwd) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let mut ctx = db::begin().await?;
    let user = user_dao::select_one(&mut ctx, login.id).await?;
    let hash = user.pwd.unwrap_or_default();
    if hash.is_empty() || !pwd_util::verify(&form.old_pwd, &hash).await {
        return Err(BizError::field("oldPwd", "当前密码不正确"));
    }
    let pwd = pwd_util::hash(&form.pwd).await;
    user_dao::update_pwd(&mut ctx, login.id, pwd).await?;
    user_login_dao::delete_others(&mut ctx, login.id, login.token.clone()).await?;
    ctx.commit().await?;
    Ok(())
}

/// 注销账号:账号不能再登录,邮箱可以重新注册
#[POST("/api/user/unregister")]
pub async fn unregister(login: Ext<LoginUser>, form: form::Pwd) -> Result<(), BizError> {
    if login.is_temp {
        return Err(biz_error!("临时账号无需注销,卸载软件即可"));
    }
    let mut ctx = db::begin().await?;
    let user = user_dao::select_one(&mut ctx, login.id).await?;
    let hash = user.pwd.unwrap_or_default();
    if hash.is_empty() || !pwd_util::verify(&form.pwd, &hash).await {
        return Err(BizError::field("pwd", "密码不正确"));
    }
    let suffix = format!("_{}", time_util::current_millis());
    user_dao::unregister(&mut ctx, login.id, suffix).await?;
    user_login_dao::delete_by_user(&mut ctx, login.id).await?;
    ctx.commit().await?;
    Ok(())
}

/// 退出登录
#[DELETE("/api/user/logout")]
pub async fn logout(headers: &HeaderMap) -> Result<(), BizError> {
    if let Some(token) = user_auth::get_token(headers) {
        let mut ctx = lib_db::get_context();
        user_login_dao::delete_by_token(&mut ctx, token.to_string()).await?;
    }
    Ok(())
}

/// 发送邮箱验证码
#[POST("/api/email/send_code")]
pub async fn send_code(form: form::SendCode) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let email_type = match form.r#type.as_str() {
        "register" => {
            if !config_service::get().allow_register {
                return Err(biz_error!("暂未开放注册"));
            }
            email_service::TYPE_REGISTER
        }
        "reset_pwd" => {
            //邮箱没有注册时也返回成功,不发送邮件,防止被用来查询邮箱是否注册过
            let mut ctx = lib_db::get_context();
            let user = user_dao::select_by_email(&mut ctx, form.email.clone()).await?;
            if !user.is_some_and(|it| it.tempFlag == 0 && it.state != 2) {
                return Ok(());
            }
            email_service::TYPE_RESET_PWD
        }
        "bind_email" => email_service::TYPE_BIND_EMAIL,
        _ => return Err(BizError::field("type", "验证码类型不正确")),
    };
    email_service::send_code(&form.email, email_type).await
}
