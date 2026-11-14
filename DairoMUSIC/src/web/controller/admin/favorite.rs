//! 歌单列表:首页(发现页)展示的歌单从会员的歌单里选择,这里查看歌单、修改歌单所属的会员

use crate::dao::{music_dao, user_dao, user_favorite_dao};
use crate::service::dfs_service;
use crate::service::file_clean_service::{self, TEMP_LOGO_PREFIX};
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::web::model::{non_empty, PageData, PageForm};
use crate::{biz_error, db};
use hyperoute_core::Query;
use hyperoute_macro::{GET, POST};
use serde::Serialize;

/// 歌单
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteItem {
    pub id: i64,
    pub name: String,

    // 封面的访问地址
    pub logo_url: Option<String>,

    // 歌曲数
    pub music_count: i64,

    // 所属会员ID,0:归管理员
    pub user_id: i64,

    // 所属会员的邮箱
    pub user_email: Option<String>,

    // 被首页的推荐分组、轮播使用的次数
    pub home_count: i64,

    // 创建时间
    pub date: i64,
}

/// 封面的访问地址
pub fn logo_url(logo: Option<&str>) -> Option<String> {
    dfs_service::file_url(logo.map(|it| it.strip_prefix(TEMP_LOGO_PREFIX).unwrap_or(it)))
}

/// 歌单列表
#[GET("/admin/api/favorite/list")]
pub async fn list(page: Query<PageForm>, id: Option<i64>, name: Option<String>, user_id: Option<i64>, home: Option<i64>) -> Result<PageData<FavoriteItem>, BizError> {
    let page_in = page.to_page_in(&["id", "name", "musicCount", "userId", "homeCount", "date"], "id", "desc");
    let mut ctx = lib_db::get_context();
    let (total, list) = user_favorite_dao::page_admin(&mut ctx, page_in, id, non_empty(name), user_id, home.filter(|it| *it == 1)).await?;
    let list = list
        .into_iter()
        .map(|it| FavoriteItem {
            logo_url: logo_url(it.logo.as_deref()),
            id: it.id,
            name: it.name,
            music_count: it.musicCount,
            user_id: it.userId,
            user_email: it.userEmail,
            home_count: it.homeCount,
            date: it.date,
        })
        .collect();
    Ok(PageData::new(total, list))
}

/// 选择歌单、歌曲时的搜索结果
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickItem {
    pub id: i64,
    pub name: String,

    // 封面的访问地址
    pub logo_url: Option<String>,

    // 补充说明(歌单:歌曲数和所属会员,歌曲:歌手)
    pub memo: String,
}

/// 所属会员的显示文字
fn owner_text(user_id: i64, email: Option<&str>) -> String {
    match (user_id, email) {
        (0, _) => "归管理员".to_string(),
        (id, Some(email)) => format!("{}(ID:{})", email, id),
        (id, None) => format!("会员ID:{}", id),
    }
}

/// 搜索歌单(添加轮播、推荐分组时选择歌单),关键词是歌单名或歌单ID
/// 放在common下:只要登录了后台就可以使用,不需要「歌单列表」的权限
#[GET("/admin/api/common/favorite_search")]
pub async fn search(key: Option<String>) -> Result<Vec<PickItem>, BizError> {
    let key = non_empty(key);
    let id = key.as_deref().and_then(|it| it.parse::<i64>().ok()).unwrap_or(0);
    let mut ctx = lib_db::get_context();
    let list = user_favorite_dao::search_admin(&mut ctx, key, id)
        .await?
        .into_iter()
        .map(|it| PickItem {
            logo_url: logo_url(it.logo.as_deref()),
            memo: format!("{}首 {}", it.musicCount, owner_text(it.userId, it.userEmail.as_deref())),
            id: it.id,
            name: it.name,
        })
        .collect();
    Ok(list)
}

/// 搜索歌曲(添加轮播时选择歌曲),关键词是歌名、歌手或歌曲ID
#[GET("/admin/api/common/music_search")]
pub async fn music_search(key: Option<String>) -> Result<Vec<PickItem>, BizError> {
    let key = non_empty(key);
    let id = key.as_deref().and_then(|it| it.parse::<i64>().ok()).unwrap_or(0);
    let mut ctx = lib_db::get_context();
    let list = music_dao::search_admin(&mut ctx, key, id)
        .await?
        .into_iter()
        .map(|it| PickItem {
            logo_url: dfs_service::file_url(it.logo.as_deref()),
            memo: it.artist.unwrap_or_default(),
            id: it.id,
            name: it.name.unwrap_or_default(),
        })
        .collect();
    Ok(list)
}

/// 修改歌单所属的会员:分配给一个会员维护,或者收回归管理员
#[POST("/admin/api/favorite/assign")]
pub async fn assign(form: form::FavoriteAssign) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    let favorite = match user_favorite_dao::select_one(&mut ctx, form.id).await {
        Ok(it) if it.isDelete == 0 && it.likeFlag == 0 => it,
        _ => return Err(biz_error!("歌单不存在")),
    };

    //会员ID或邮箱,空或0表示归管理员
    let text = form.user.trim();
    let user_id = if text.is_empty() || text == "0" {
        0
    } else {
        let user = match text.parse::<i64>() {
            Ok(id) => user_dao::select_one(&mut ctx, id).await.ok(),
            Err(_) => user_dao::select_by_email(&mut ctx, text.to_string()).await?,
        };
        let Some(user) = user.filter(|it| it.state == 1) else {
            return Err(BizError::field("user", "会员不存在或不是有效状态"));
        };
        if user_favorite_dao::count_by_name(&mut ctx, user.id, favorite.name.clone(), favorite.id).await? > 0 {
            return Err(BizError::field("user", "这个会员已经有同名的歌单"));
        }
        user.id
    };
    user_favorite_dao::set_owner(&mut ctx, form.id, user_id).await?;
    ctx.commit().await?;
    Ok(())
}

/// 删除归管理员的歌单(首页正在使用的不能删除)
#[POST("/admin/api/favorite/delete")]
pub async fn delete(id: i64) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    let favorite = match user_favorite_dao::select_one(&mut ctx, id).await {
        Ok(it) if it.isDelete == 0 => it,
        _ => return Err(biz_error!("歌单不存在")),
    };
    if favorite.userId != 0 {
        return Err(biz_error!("只能删除归管理员的歌单,会员的歌单由会员自己删除"));
    }
    if user_favorite_dao::count_referenced(&mut ctx, id.to_string()).await? > 0 {
        return Err(biz_error!("首页的推荐分组或轮播正在使用这个歌单,请先从首页移除"));
    }
    user_favorite_dao::set_deleted(&mut ctx, id, time_util::current_millis()).await?;
    ctx.commit().await?;

    //上传的封面从DairoDFS删除
    file_clean_service::remove_replaced(favorite.logo, None);
    Ok(())
}
