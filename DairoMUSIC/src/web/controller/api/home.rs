//! 客户端:首页(轮播、推荐歌单)

use crate::dao::{swiper_dao, useful_dao, user_favorite_dao};
use crate::service::dfs_service;
use crate::web::biz_error::BizError;
use crate::web::controller::api::favorite::FavoriteVo;
use crate::web::filter::user_auth::LoginUser;
use hyperoute_core::Ext;
use hyperoute_macro::GET;
use serde::Serialize;
use std::collections::HashMap;

/// 轮播
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwiperVo {
    pub id: i64,
    pub name: String,

    // 图片地址
    pub image: Option<String>,

    // 类型 1:推荐的歌单(data为歌单ID) 2:推荐单曲(data为歌曲ID) 3:拨打电话 4:跳转网页
    pub r#type: i64,
    pub data: Option<String>,
}

/// 推荐歌单分组
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsefulVo {
    // 分组名
    pub name: Option<String>,

    // 显示方式 1:表格 2:横向滚动
    pub r#type: i64,

    // 歌单
    pub favorites: Vec<FavoriteVo>,
}

/// 轮播
#[GET("/api/home/swiper")]
pub async fn swiper() -> Result<Vec<SwiperVo>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = swiper_dao::select_active(&mut ctx)
        .await?
        .into_iter()
        .map(|it| SwiperVo {
            id: it.id,
            name: it.name,
            image: dfs_service::file_url(Some(&it.image)),
            r#type: it.swiperType,
            data: it.data,
        })
        .collect();
    Ok(list)
}

/// 推荐歌单
#[GET("/api/home/useful")]
pub async fn useful(login: Ext<LoginUser>) -> Result<Vec<UsefulVo>, BizError> {
    let mut ctx = lib_db::get_context();
    let groups = useful_dao::select_active(&mut ctx).await?;
    let parse_ids = |text: &str| -> Vec<i64> { text.split(',').filter_map(|it| it.trim().parse().ok()).collect() };
    let mut all_ids: Vec<i64> = groups.iter().flat_map(|it| parse_ids(&it.favoriteIds)).collect();
    all_ids.sort();
    all_ids.dedup();
    let favorites = if all_ids.is_empty() {
        Vec::new()
    } else {
        user_favorite_dao::select_by_ids(&mut ctx, all_ids).await?
    };
    let mut favorite_map: HashMap<i64, _> = favorites.into_iter().map(|it| (it.id, it)).collect();
    let mut result = Vec::new();
    for group in groups {
        let favorites: Vec<FavoriteVo> = parse_ids(&group.favoriteIds)
            .into_iter()
            .filter_map(|id| favorite_map.get(&id).cloned())
            .map(|it| FavoriteVo::new(it, login.id))
            .collect();
        if favorites.is_empty() {
            continue;
        }
        result.push(UsefulVo {
            name: group.name,
            r#type: group.showType,
            favorites,
        });
    }
    favorite_map.clear();
    Ok(result)
}
