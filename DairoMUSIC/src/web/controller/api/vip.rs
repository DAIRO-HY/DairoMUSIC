//! 客户端:VIP

use crate::dao::{user_dao, vip_code_dao};
use crate::service::config_service;
use crate::service::vip_service::{self, VipVo, DAY_MILLIS};
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::filter::user_auth::LoginUser;
use crate::web::form;
use crate::{biz_error, biz_errorf, db};
use hyperoute_core::Ext;
use hyperoute_macro::POST;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// 一个会员在一段时间内最多可以输错几次(防止猜卡密)
const FAIL_LIMIT: i64 = 10;

/// 统计输错次数的时间段(毫秒)
const FAIL_WINDOW: i64 = 3600 * 1000;

/// 输错的次数: 会员ID -> (次数, 第一次输错的时间)
static FAILS: LazyLock<Mutex<HashMap<i64, (i64, i64)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// 同一时间只处理一个兑换,避免一个兑换码被同时兑换两次
static REDEEM_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 兑换结果
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemResult {
    // 是否需要会员确认:为true时兑换码还没有被使用,客户端显示message让会员确认,确认之后带上confirm=1再提交一次
    pub need_confirm: bool,

    // 提示信息
    pub message: String,

    // 兑换之后的VIP状态
    pub vip: VipVo,
}

/// 这个会员是否输错太多次
fn is_fail_limited(user_id: i64) -> bool {
    let now = time_util::current_millis();
    let mut map = FAILS.lock().unwrap();

    //顺便清理已经过了时间段的记录
    map.retain(|_, (_, start)| now - *start < FAIL_WINDOW);
    map.get(&user_id).is_some_and(|(count, _)| *count >= FAIL_LIMIT)
}

/// 记录一次输错
fn add_fail(user_id: i64) {
    let now = time_util::current_millis();
    let mut map = FAILS.lock().unwrap();
    map.entry(user_id).or_insert((0, now)).0 += 1;
}

/// 用兑换码(卡号+卡密)兑换VIP
/// - 不是VIP、或者VIP已经到期:从现在开始计算
/// - 已经是VIP:在到期时间上累加,累加之后超过后台设置的最长年数时不能兑换(兑换码不会被使用)
/// - 永久的兑换码:成为永久VIP
/// - 已经是永久VIP:兑换没有实际效果,只是消耗兑换码,需要会员确认(confirm=1)之后才使用兑换码
#[POST("/api/vip/redeem")]
pub async fn redeem(login: Ext<LoginUser>, form: form::VipRedeem) -> Result<RedeemResult, BizError> {
    if !vip_service::is_enabled(login.is_test) {
        return Err(biz_error!("VIP功能没有开启"));
    }

    //卡号可以带空格或者横线(每4位一组显示时),卡密不区分大小写
    let card_no: String = form.card_no.chars().filter(|it| !it.is_whitespace() && *it != '-').collect();
    let card_pwd = form.card_pwd.trim().to_uppercase();
    if card_no.is_empty() {
        return Err(BizError::field("cardNo", "请输入卡号"));
    }
    if card_pwd.is_empty() {
        return Err(BizError::field("cardPwd", "请输入卡密"));
    }
    if is_fail_limited(login.id) {
        return Err(biz_error!("输错的次数太多,请1小时之后再试"));
    }
    let _lock = REDEEM_LOCK.lock().await;
    let now = time_util::current_millis();
    let mut ctx = db::begin().await?;
    let code = vip_code_dao::select_by_card_no(&mut ctx, card_no).await?;
    let Some(code) = code.filter(|it| it.cardPwd == card_pwd) else {
        add_fail(login.id);
        return Err(biz_error!("卡号或卡密不正确"));
    };
    match code.state {
        0 => {}
        1 => return Err(biz_error!("这个兑换码已经被使用")),
        _ => return Err(biz_error!("这个兑换码已经作废")),
    }
    if code.expireTime < now {
        return Err(biz_error!("这个兑换码已经过期"));
    }
    let mut user = user_dao::select_one(&mut ctx, login.id).await?;
    let message;
    if user.vipForever == 1 {
        if form.confirm != 1 {
            return Ok(RedeemResult {
                need_confirm: true,
                message: "你已经是永久VIP,继续兑换只会消耗这个兑换码,没有实际效果。确定要兑换吗?".to_string(),
                vip: vip_service::vo(&user),
            });
        }
        message = "兑换码已经使用,你已经是永久VIP".to_string();
    } else if code.days == 0 {
        user.vipForever = 1;
        message = "兑换成功,你已经成为永久VIP".to_string();
    } else {
        let is_vip = user.vipTime > now;
        let vip_time = user.vipTime.max(now) + code.days * DAY_MILLIS;
        let max_years = config_service::get().vip_max_years.max(1);

        //不是VIP的会员总是可以兑换,否则时长超过最长年数的兑换码谁都用不了
        if is_vip && vip_time > now + max_years * 365 * DAY_MILLIS {
            return Err(biz_errorf!("VIP的有效期最多只能累加到{}年,现在还不能使用这个兑换码", max_years));
        }
        user.vipTime = vip_time;
        message = format!("兑换成功,VIP有效期到{}", time_util::date_text(vip_time));
    }
    vip_code_dao::set_used(&mut ctx, code.id, login.id, now).await?;
    user_dao::update_vip(&mut ctx, login.id, user.vipTime, user.vipForever).await?;
    ctx.commit().await?;
    Ok(RedeemResult {
        need_confirm: false,
        message,
        vip: vip_service::vo(&user),
    })
}
