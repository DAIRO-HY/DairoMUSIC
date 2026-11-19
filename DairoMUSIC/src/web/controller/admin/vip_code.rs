//! VIP兑换码:批量生成、查看、作废、导出

use crate::dao::vip_code_dao::{self, PageListParam, VipCode};
use crate::service::vip_service;
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::web::model::{non_empty, PageData, PageForm};
use crate::{biz_error, db};
use hyperoute_core::Query;
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use std::collections::HashSet;
use validator::Validate;

/// 状态筛选:已过期(没有使用,并且过了有效期)
const STATE_EXPIRED: i64 = 3;

/// 兑换码
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VipCodeItem {
    pub id: i64,
    pub card_no: String,
    pub card_pwd: String,
    pub batch_no: String,

    // 兑换之后VIP的天数,0:永久
    pub days: i64,

    // 兑换码的有效期
    pub expire_time: i64,

    // 状态 0:未使用 1:已使用 2:已作废
    pub state: i64,

    // 是否已经过期(没有使用,并且过了有效期)
    pub expired: bool,

    // 兑换的会员
    pub user_id: Option<i64>,
    pub user_email: Option<String>,
    pub user_name: Option<String>,

    // 兑换时间
    pub use_time: Option<i64>,

    // 生成时间
    pub date: i64,

    // 标签
    pub tag: Option<String>,
}

/// 兑换码明细
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VipCodeDetailVo {
    #[serde(flatten)]
    pub item: VipCodeItem,

    // 备注
    pub memo: Option<String>,
}

/// 导出的兑换码
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VipCodeExport {
    pub card_no: String,
    pub card_pwd: String,
    pub days: i64,
    pub expire_time: i64,
    pub state: i64,
}

/// 兑换码列表
/// - state 0:未使用(还没有过期) 1:已使用 2:已作废 3:已过期
#[GET("/admin/api/vip_code/list")]
pub async fn list(
    page: Query<PageForm>,
    batch_no: Option<String>,
    card_no: Option<String>,
    state: Option<i64>,
    user_id: Option<i64>,
    tag: Option<String>,
) -> Result<PageData<VipCodeItem>, BizError> {
    let page_in = page.to_page_in(&["id", "cardNo", "batchNo", "days", "expireTime", "state", "useTime", "tag"], "id", "desc");
    let now = time_util::current_millis();
    let (state, expire_before, expire_after) = match state {
        Some(0) => (Some(0), None, Some(now)),
        Some(STATE_EXPIRED) => (Some(0), Some(now), None),
        it => (it, None, None),
    };

    //卡号可以带空格或者横线
    let card_no = non_empty(card_no).map(|it| it.chars().filter(|c| !c.is_whitespace() && *c != '-').collect::<String>());
    let mut ctx = lib_db::get_context();
    let param = PageListParam {
        batch_no: non_empty(batch_no),
        card_no,
        state,
        user_id,
        expire_before,
        expire_after,
        tag: non_empty(tag),
    };
    let (total, list) = vip_code_dao::page_list(&mut ctx, page_in, param).await?;
    let list = list
        .into_iter()
        .map(|it| VipCodeItem {
            expired: it.state == 0 && it.expireTime < now,
            id: it.id,
            card_no: it.cardNo,
            card_pwd: it.cardPwd,
            batch_no: it.batchNo,
            days: it.days,
            expire_time: it.expireTime,
            state: it.state,
            user_id: it.userId,
            user_email: it.userEmail,
            user_name: it.userName,
            use_time: it.useTime,
            date: it.date,
            tag: it.tag,
        })
        .collect();
    Ok(PageData::new(total, list))
}

/// 兑换码明细
#[GET("/admin/api/vip_code/detail")]
pub async fn detail(id: i64) -> Result<VipCodeDetailVo, BizError> {
    let mut ctx = lib_db::get_context();
    let Some(it) = vip_code_dao::select_detail(&mut ctx, id).await? else {
        return Err(biz_error!("兑换码不存在"));
    };
    let now = time_util::current_millis();
    Ok(VipCodeDetailVo {
        memo: it.memo,
        item: VipCodeItem {
            expired: it.state == 0 && it.expireTime < now,
            id: it.id,
            card_no: it.cardNo,
            card_pwd: it.cardPwd,
            batch_no: it.batchNo,
            days: it.days,
            expire_time: it.expireTime,
            state: it.state,
            user_id: it.userId,
            user_email: it.userEmail,
            user_name: it.userName,
            use_time: it.useTime,
            date: it.date,
            tag: it.tag,
        },
    })
}

/// 修改兑换码:标签、备注任何状态都可以修改;VIP时长、有效期只有未使用的可以修改(已使用、已作废的不修改)
#[POST("/admin/api/vip_code/edit")]
pub async fn edit(form: form::VipCodeEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let mut ctx = lib_db::get_context();
    let Some(code) = vip_code_dao::select_detail(&mut ctx, form.id).await? else {
        return Err(biz_error!("兑换码不存在"));
    };
    let validity = if code.state == 0 { Some(check_validity(form.forever, form.days, &form.expire_date)?) } else { None };
    let mut ctx = db::begin().await?;
    if let Some((days, expire_time)) = validity {
        vip_code_dao::update_validity(&mut ctx, form.id, days, expire_time).await?;
    }
    vip_code_dao::update_info(&mut ctx, form.id, non_empty(Some(form.tag)), non_empty(Some(form.memo))).await?;
    ctx.commit().await?;
    Ok(())
}

/// 检查VIP时长和有效期,返回VIP天数(0:永久)和有效期(毫秒)
fn check_validity(forever: i64, days: i64, expire_date: &str) -> Result<(i64, i64), BizError> {
    let days = if forever == 1 { 0 } else { days };
    if forever != 1 && !(1..=36500).contains(&days) {
        return Err(BizError::field("days", "VIP时长必须在1到36500天之间"));
    }
    let Some(expire_time) = time_util::date_end_millis(expire_date) else {
        return Err(BizError::field("expireDate", "请选择兑换码的有效期"));
    };
    if expire_time < time_util::current_millis() {
        return Err(BizError::field("expireDate", "有效期不能早于今天"));
    }
    Ok((days, expire_time))
}

/// 批量生成兑换码,返回批次号(生成时间,如20261114123400)
#[POST("/admin/api/vip_code/generate")]
pub async fn generate(form: form::VipCodeGenerate) -> Result<String, BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let (days, expire_time) = check_validity(form.forever, form.days, &form.expire_date)?;
    let now = time_util::current_millis();
    let tag = non_empty(Some(form.tag.clone()));
    let memo = non_empty(Some(form.memo.clone()));
    let batch_no = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let mut ctx = db::begin().await?;
    if vip_code_dao::count_by_batch(&mut ctx, batch_no.clone()).await? > 0 {
        //批次号精确到秒,同一秒内生成了两次
        return Err(biz_error!("操作太快,请稍后再试"));
    }
    let mut card_nos = HashSet::new();
    while card_nos.len() < form.count as usize {
        let card_no = vip_service::new_card_no();

        //卡号不能重复:这一批里没有,数据库里也没有
        if card_nos.contains(&card_no) || vip_code_dao::select_by_card_no(&mut ctx, card_no.clone()).await?.is_some() {
            continue;
        }
        let code = VipCode {
            cardNo: card_no.clone(),
            cardPwd: vip_service::new_card_pwd(form.pwd_length as usize),
            batchNo: batch_no.clone(),
            days,
            expireTime: expire_time,
            date: now,
            tag: tag.clone(),
            memo: memo.clone(),
            ..Default::default()
        };
        vip_code_dao::insert(&mut ctx, &code).await?;
        card_nos.insert(card_no);
    }
    ctx.commit().await?;
    Ok(batch_no)
}

/// 作废没有使用的兑换码
/// - ids 兑换码ID,逗号分隔
#[POST("/admin/api/vip_code/void")]
pub async fn void(ids: String) -> Result<(), BizError> {
    let mut id_list = Vec::new();
    for it in ids.split(',').map(str::trim).filter(|it| !it.is_empty()) {
        let Ok(id) = it.parse::<i64>() else {
            return Err(biz_error!("兑换码ID格式错误"));
        };
        if !id_list.contains(&id) {
            id_list.push(id);
        }
    }

    //没有ID时会作废所有兑换码,必须检查
    if id_list.is_empty() {
        return Err(biz_error!("请选择要作废的兑换码"));
    }
    if id_list.len() > 1000 {
        return Err(biz_error!("一次最多作废1000个兑换码"));
    }
    let mut ctx = lib_db::get_context();
    vip_code_dao::set_void(&mut ctx, id_list).await?;
    Ok(())
}

/// 作废整个批次里没有使用的兑换码
#[POST("/admin/api/vip_code/void_batch")]
pub async fn void_batch(batch_no: String) -> Result<(), BizError> {
    let batch_no = batch_no.trim().to_string();
    let mut ctx = lib_db::get_context();
    if batch_no.is_empty() || vip_code_dao::count_by_batch(&mut ctx, batch_no.clone()).await? == 0 {
        return Err(biz_error!("批次号不存在"));
    }
    vip_code_dao::set_void_batch(&mut ctx, batch_no).await?;
    Ok(())
}

/// 导出一个批次的所有兑换码(页面上生成CSV文件)
#[GET("/admin/api/vip_code/export")]
pub async fn export(batch_no: String) -> Result<Vec<VipCodeExport>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = vip_code_dao::select_by_batch(&mut ctx, batch_no.trim().to_string()).await?;
    if list.is_empty() {
        return Err(biz_error!("批次号不存在"));
    }
    let now = time_util::current_millis();
    Ok(list
        .into_iter()
        .map(|it| VipCodeExport {
            //已过期的在页面上显示成"已过期"
            state: if it.state == 0 && it.expireTime < now { STATE_EXPIRED } else { it.state },
            card_no: it.cardNo,
            card_pwd: it.cardPwd,
            days: it.days,
            expire_time: it.expireTime,
        })
        .collect())
}
