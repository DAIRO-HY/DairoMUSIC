//! VIP:后台"系统配置"里开启之后,不是VIP的会员只能听、下载标准音质的歌曲和流畅版的视频,不能把文件发送到APP外面、保存到相册
//! 会员通过后台生成的兑换码(卡号+卡密)成为VIP

use crate::dao::user_dao::User;
use crate::service::config_service;
use crate::util::time_util;
use crate::web::filter::user_auth::LoginUser;
use rand::RngExt;
use serde::Serialize;

/// 一天的毫秒数
pub const DAY_MILLIS: i64 = 24 * 3600 * 1000;

/// 卡号里随机数的位数(前面还有6位日期)
const CARD_NO_RANDOM_LENGTH: usize = 10;

/// 卡密使用的字符:大写字母和数字,去掉容易看错的0、O、1、I、L
const CARD_PWD_CHARS: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

/// 会员的VIP状态
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VipVo {
    // 这个会员是否受VIP功能的影响(后台没有开启VIP功能时为false,这时所有人都没有限制,客户端不显示VIP相关的内容)
    pub enable: bool,

    // 现在是否VIP
    pub is_vip: bool,

    // 是否永久VIP
    pub forever: bool,

    // VIP到期时间(毫秒);0:从来不是VIP;永久VIP时没有意义
    pub expire_time: i64,
}

/// 现在是否VIP
pub fn is_vip(user: &User) -> bool {
    user.vipForever == 1 || user.vipTime > time_util::current_millis()
}

/// VIP功能是否对这个会员生效
/// 应用市场审核用的测试会员不生效:应用市场不允许用兑换码解锁功能,审核时看不到VIP相关的内容
pub fn is_enabled(is_test: bool) -> bool {
    config_service::get().vip_enable && !is_test
}

/// 这个会员是否受到非VIP的限制(只能听标准音质、看流畅版视频)
pub fn is_limited(login: &LoginUser) -> bool {
    is_enabled(login.is_test) && !login.is_vip
}

/// 没有登录的访问(分享页)是否受到非VIP的限制
pub fn is_guest_limited() -> bool {
    config_service::get().vip_enable
}

/// 会员的VIP状态
pub fn vo(user: &User) -> VipVo {
    VipVo {
        enable: is_enabled(user.testFlag == 1),
        is_vip: is_vip(user),
        forever: user.vipForever == 1,
        expire_time: user.vipTime,
    }
}

/// 生成卡号:6位日期(年月日)+10位随机数,一共16位数字
pub fn new_card_no() -> String {
    let mut rng = rand::rng();
    let mut card_no = chrono::Local::now().format("%y%m%d").to_string();
    for _ in 0..CARD_NO_RANDOM_LENGTH {
        card_no.push(char::from(b'0' + rng.random_range(0..10u8)));
    }
    card_no
}

/// 生成卡密
pub fn new_card_pwd(length: usize) -> String {
    let mut rng = rand::rng();
    (0..length).map(|_| char::from(CARD_PWD_CHARS[rng.random_range(0..CARD_PWD_CHARS.len())])).collect()
}
