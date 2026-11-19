//! 歌单列表:首页(发现页)展示的歌单从会员的歌单里选择,这里查看歌单、修改歌单所属的会员
//! 管理员也可以自己建歌单(归管理员),编辑歌单名、简介、封面和歌曲

use crate::dao::user_favorite_dao::UserFavorite;
use crate::dao::user_music_dao::UserMusic;
use crate::dao::{music_dao, user_dao, user_favorite_dao, user_music_dao};
use crate::web::controller::api::favorite::MAX_MUSIC_COUNT;
use crate::web::controller::api::user::upload_image;
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
use sqlx_context::DbContext;
use validator::Validate;

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
    user_music_dao::update_user_by_favorite(&mut ctx, form.id, user_id).await?;
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

/// 歌单详情(编辑页)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteDetail {
    pub id: i64,
    pub name: String,

    // 简介
    pub description: Option<String>,

    // 封面的访问地址
    pub logo_url: Option<String>,

    // 是否上传了封面(false:使用第一首歌的封面)
    pub custom_logo: bool,

    // 歌单里的歌曲,按显示顺序排列
    pub musics: Vec<PickItem>,
}

/// 归管理员的歌单(只有归管理员的歌单可以在后台编辑,会员的歌单由会员自己维护)
async fn get_admin_favorite(ctx: &mut DbContext, id: i64) -> Result<UserFavorite, BizError> {
    match user_favorite_dao::select_one(ctx, id).await {
        Ok(it) if it.isDelete == 0 && it.likeFlag == 0 => {
            if it.userId != 0 {
                return Err(biz_error!("只能编辑归管理员的歌单,会员的歌单由会员自己维护"));
            }
            Ok(it)
        }
        _ => Err(biz_error!("歌单不存在")),
    }
}

/// 歌单详情
#[GET("/admin/api/favorite/detail")]
pub async fn detail(id: i64) -> Result<FavoriteDetail, BizError> {
    let mut ctx = lib_db::get_context();
    let favorite = get_admin_favorite(&mut ctx, id).await?;
    let musics = user_music_dao::select_music_by_favorite(&mut ctx, id, 0)
        .await?
        .into_iter()
        .map(|it| PickItem {
            logo_url: dfs_service::file_url(it.logo.as_deref()),
            memo: it.artist.unwrap_or_default(),
            id: it.id,
            name: it.name.unwrap_or_default(),
        })
        .collect();
    Ok(FavoriteDetail {
        custom_logo: favorite.logo.as_deref().is_some_and(|it| !it.starts_with(TEMP_LOGO_PREFIX)),
        logo_url: logo_url(favorite.logo.as_deref()),
        id: favorite.id,
        name: favorite.name,
        description: favorite.description,
        musics,
    })
}

/// 新建、修改归管理员的歌单,歌曲按提交的顺序全部替换,返回歌单ID
/// 没有上传封面时使用第一首歌的封面
#[POST("/admin/api/favorite/save")]
pub async fn save(form: form::FavoriteSave) -> Result<i64, BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let name = form.name.trim().to_string();
    if name.is_empty() {
        return Err(BizError::field("name", "请输入歌单名"));
    }
    let mut ids = Vec::new();
    for it in form.music_ids.split(',').map(str::trim).filter(|it| !it.is_empty()) {
        let Ok(id) = it.parse::<i64>() else {
            return Err(BizError::field("musicIds", "歌曲ID格式错误"));
        };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.len() as i64 > MAX_MUSIC_COUNT {
        return Err(BizError::field("musicIds", &format!("一个歌单最多只能添加{}首歌", MAX_MUSIC_COUNT)));
    }

    let mut ctx = db::begin().await?;
    let old = if form.id == 0 {
        None
    } else {
        Some(get_admin_favorite(&mut ctx, form.id).await?)
    };
    if user_favorite_dao::count_by_name(&mut ctx, 0, name.clone(), form.id).await? > 0 {
        return Err(BizError::field("name", "归管理员的歌单里已经有这个名称"));
    }
    let mut musics = Vec::with_capacity(ids.len());
    for id in ids {
        match music_dao::select_one(&mut ctx, id).await {
            Ok(it) if it.deleteFlag == 0 => musics.push(it),
            _ => return Err(BizError::field("musicIds", &format!("歌曲不存在或已删除(ID:{})", id))),
        }
    }
    let description = non_empty(Some(form.description));
    let id = match &old {
        None => {
            let favorite = UserFavorite {
                name,
                description,
                date: time_util::current_millis(),
                userId: 0,
                ..Default::default()
            };
            user_favorite_dao::insert(&mut ctx, &favorite).await?
        }
        Some(it) => {
            user_favorite_dao::update_info(&mut ctx, it.id, name, description).await?;
            it.id
        }
    };

    //排序号越大越靠前
    user_music_dao::delete_by_favorite(&mut ctx, id).await?;
    let count = musics.len() as i64;
    for (i, it) in musics.iter().enumerate() {
        let user_music = UserMusic {
            favoriteId: id,
            musicId: it.id,
            userId: 0,
            nick: None,
            sort: count - i as i64,
        };
        user_music_dao::insert(&mut ctx, &user_music).await?;
    }
    user_favorite_dao::refresh_music_count(&mut ctx, id).await?;

    //没有上传封面时,用第一首歌的封面
    let old_logo = old.and_then(|it| it.logo);
    if old_logo.as_deref().is_none_or(|it| it.starts_with(TEMP_LOGO_PREFIX)) {
        let logo = musics.iter().find_map(|it| it.logo.as_ref()).map(|it| format!("{}{}", TEMP_LOGO_PREFIX, it));
        user_favorite_dao::update_logo(&mut ctx, id, logo).await?;
    }
    ctx.commit().await?;
    Ok(id)
}

/// 封面上传
pub struct LogoUpload {
    // 歌单ID
    pub id: i64,

    // 图片内容
    pub file: Vec<u8>,
}

/// 上传归管理员的歌单的封面,返回封面地址
#[POST("/admin/api/favorite/upload_logo")]
pub async fn upload_logo(form: LogoUpload) -> Result<Option<String>, BizError> {
    let mut ctx = lib_db::get_context();
    let favorite = get_admin_favorite(&mut ctx, form.id).await?;
    let path = upload_image(form.file).await?;
    user_favorite_dao::update_logo(&mut ctx, form.id, Some(path.clone())).await?;
    file_clean_service::remove_replaced(favorite.logo, Some(&path));
    Ok(dfs_service::file_url(Some(&path)))
}
