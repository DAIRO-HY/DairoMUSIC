//! 重复歌曲处理

use crate::dao::music_dao::{PageDuplicateTemp, SelectSameTemp};
use crate::dao::music_dao;
use crate::service::{dfs_service, music_service};
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::web::model::{non_empty, PageData, PageForm};
use crate::{biz_error, db};
use hyperoute_core::Query;
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use std::collections::BTreeSet;

/// 同名的歌曲
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SameMusic {
    #[serde(flatten)]
    pub music: SelectSameTemp,

    // 封面的访问地址
    pub logo_url: Option<String>,
}

/// 重复歌曲列表
/// - ignore_artist 1:只按歌名判断重复
#[GET("/admin/api/music_duplicate/list")]
pub async fn list(page: Query<PageForm>, name: Option<String>, artist: Option<String>, ignore_artist: Option<i64>) -> Result<PageData<PageDuplicateTemp>, BizError> {
    let page_in = page.to_page_in(&["name", "artist", "count"], "count", "desc");
    let mut ctx = lib_db::get_context();
    if ignore_artist == Some(1) {
        let (total, list) = music_dao::page_duplicate_ignore_artist(&mut ctx, page_in, non_empty(name)).await?;
        let list = list
            .into_iter()
            .map(|it| PageDuplicateTemp {
                name: it.name,
                artist: None,
                count: it.count,
            })
            .collect();
        return Ok(PageData::new(total, list));
    }
    let (total, list) = music_dao::page_duplicate(&mut ctx, page_in, non_empty(name), non_empty(artist)).await?;
    Ok(PageData::new(total, list))
}

/// 歌名(和歌手)相同的歌曲
#[GET("/admin/api/music_duplicate/same")]
pub async fn same(name: String, artist: Option<String>, ignore_artist: Option<i64>) -> Result<Vec<SameMusic>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = music_dao::select_same(&mut ctx, name, artist, ignore_artist.unwrap_or(0))
        .await?
        .into_iter()
        .map(|it| SameMusic {
            logo_url: dfs_service::file_url(it.logo.as_deref()),
            music: it,
        })
        .collect();
    Ok(list)
}

/// 去重:保留一首,其他的标记为删除,收藏了被删除歌曲的歌单改为收藏保留的歌曲
#[POST("/admin/api/music_duplicate/distinct")]
pub async fn distinct(form: form::MusicDistinct) -> Result<(), BizError> {
    let mut ids = BTreeSet::new();
    for it in form.ids.split(',').filter(|it| !it.is_empty()) {
        let Ok(id) = it.trim().parse::<i64>() else {
            return Err(biz_error!("歌曲ID格式错误"));
        };
        ids.insert(id);
    }
    ids.remove(&form.best_id);
    if ids.is_empty() {
        return Err(biz_error!("请至少选择一首要合并的歌曲"));
    }
    let ids: Vec<i64> = ids.into_iter().collect();
    let mut ctx = db::begin().await?;
    music_service::merge(&mut ctx, &ids, form.best_id).await?;
    ctx.commit().await?;
    Ok(())
}
