//! 首页(发现页)的推荐歌单分组

use crate::dao::useful_dao::Useful;
use crate::dao::{useful_dao, user_favorite_dao};
use crate::web::biz_error::BizError;
use crate::web::controller::admin::favorite::{logo_url, PickItem};
use crate::web::form;
use crate::{biz_error, db};
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use sqlx_context::DbContext;
use validator::Validate;

/// 一个分组最多的歌单数(favoriteIds字段的长度有限)
const MAX_FAVORITE_COUNT: usize = 60;

/// 推荐分组
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsefulItem {
    pub id: i64,

    // 分组名
    pub name: Option<String>,

    // 显示方式 1:表格 2:横向滚动
    pub show_type: i64,
    pub state: i64,
    pub sort: i64,

    // 分组里的歌单,按显示顺序排列(已经删除的歌单不包括)
    pub favorites: Vec<PickItem>,
}

/// 逗号分隔的歌单ID
fn parse_ids(text: &str) -> Vec<i64> {
    let mut ids = Vec::new();
    for id in text.split(',').filter_map(|it| it.trim().parse::<i64>().ok()) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

async fn to_item(ctx: &mut DbContext, it: Useful) -> Result<UsefulItem, BizError> {
    let ids = parse_ids(&it.favoriteIds);
    let list = if ids.is_empty() {
        Vec::new()
    } else {
        user_favorite_dao::select_by_ids(ctx, ids.clone()).await?
    };

    //按分组里的顺序排列
    let favorites = ids
        .iter()
        .filter_map(|id| list.iter().find(|it| it.id == *id))
        .map(|it| PickItem {
            id: it.id,
            name: it.name.clone(),
            logo_url: logo_url(it.logo.as_deref()),
            memo: format!("{}首", it.musicCount),
        })
        .collect();
    Ok(UsefulItem {
        id: it.id,
        name: it.name,
        show_type: it.showType,
        state: it.state,
        sort: it.sort,
        favorites,
    })
}

/// 推荐分组列表
#[GET("/admin/api/useful/list")]
pub async fn list() -> Result<Vec<UsefulItem>, BizError> {
    let mut ctx = lib_db::get_context();
    let mut list = Vec::new();
    for it in useful_dao::select_list(&mut ctx).await? {
        list.push(to_item(&mut ctx, it).await?);
    }
    Ok(list)
}

/// 推荐分组详情,id为0时返回新增用的默认值
#[GET("/admin/api/useful/detail")]
pub async fn detail(id: i64) -> Result<UsefulItem, BizError> {
    if id == 0 {
        return Ok(UsefulItem {
            show_type: 1,
            state: 1,
            ..Default::default()
        });
    }
    let mut ctx = lib_db::get_context();
    let Ok(useful) = useful_dao::select_one(&mut ctx, id).await else {
        return Err(biz_error!("推荐分组不存在"));
    };
    to_item(&mut ctx, useful).await
}

/// 添加或修改推荐分组
#[POST("/admin/api/useful/edit")]
pub async fn edit(form: form::UsefulEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    if ![1, 2].contains(&form.show_type) {
        return Err(BizError::field("showType", "显示方式不正确"));
    }
    if ![0, 1].contains(&form.state) {
        return Err(BizError::field("state", "状态不正确"));
    }
    let ids = parse_ids(&form.favorite_ids);
    if ids.is_empty() {
        return Err(BizError::field("favoriteIds", "请至少选择一个歌单"));
    }
    if ids.len() > MAX_FAVORITE_COUNT {
        return Err(BizError::field("favoriteIds", &format!("一个分组最多{}个歌单", MAX_FAVORITE_COUNT)));
    }
    let mut ctx = db::begin().await?;
    let favorites = user_favorite_dao::select_by_ids(&mut ctx, ids.clone()).await?;
    if favorites.len() != ids.len() || favorites.iter().any(|it| it.likeFlag == 1) {
        return Err(BizError::field("favoriteIds", "有的歌单不存在或已经被删除,请重新选择"));
    }
    let favorite_ids = ids.iter().map(|it| it.to_string()).collect::<Vec<_>>().join(",");
    let name = Some(form.name.trim().to_string()).filter(|it| !it.is_empty());
    if form.id == 0 {
        let useful = Useful {
            name,
            showType: form.show_type,
            favoriteIds: favorite_ids,
            sort: form.sort,
            state: form.state,
            ..Default::default()
        };
        useful_dao::insert(&mut ctx, &useful).await?;
    } else {
        let Ok(mut useful) = useful_dao::select_one(&mut ctx, form.id).await else {
            return Err(biz_error!("推荐分组不存在"));
        };
        useful.name = name;
        useful.showType = form.show_type;
        useful.favoriteIds = favorite_ids;
        useful.sort = form.sort;
        useful.state = form.state;
        useful_dao::update(&mut ctx, &useful).await?;
    }
    ctx.commit().await?;
    Ok(())
}

/// 删除推荐分组(分组里的歌单不会删除)
#[POST("/admin/api/useful/delete")]
pub async fn delete(id: i64) -> Result<(), BizError> {
    let mut ctx = lib_db::get_context();
    useful_dao::delete(&mut ctx, id).await?;
    Ok(())
}
