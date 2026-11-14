//! 客户端:歌单

use crate::dao::user_favorite_dao::UserFavorite;
use crate::dao::user_music_dao::{FavoriteMusicTemp, UserMusic};
use crate::dao::{music_dao, music_file_dao, user_favorite_dao, user_music_dao};
use crate::service::file_clean_service::{self, TEMP_LOGO_PREFIX};
use crate::service::{dfs_service, vip_service};
use crate::service::music_service::{self, MusicVo};
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::controller::api::user::upload_image;
use crate::web::filter::user_auth::LoginUser;
use crate::web::form;
use crate::web::model::non_empty;
use crate::{biz_error, db};
use hyperoute_core::Ext;
use hyperoute_macro::{DELETE, GET, POST};
use serde::Serialize;
use sqlx_context::DbContext;
use validator::Validate;

/// 歌单最多的歌曲数
const MAX_MUSIC_COUNT: i64 = 10000;

/// 歌单
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteVo {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,

    // 封面地址;"我喜欢"歌单为空,客户端显示固定的图标
    pub logo: Option<String>,

    // 是否"我喜欢"歌单(不能删除、不能改名)
    pub is_like: bool,

    // 歌曲数
    pub music_count: i64,

    // 是否自己的歌单
    pub is_mine: bool,
}

impl FavoriteVo {
    pub fn new(it: UserFavorite, login_id: i64) -> Self {
        let logo = if it.likeFlag == 1 {
            None
        } else {
            let path = it.logo.as_deref().map(|it| it.strip_prefix(TEMP_LOGO_PREFIX).unwrap_or(it));
            dfs_service::file_url(path)
        };
        Self {
            is_mine: it.userId == login_id,
            id: it.id,
            name: it.name,
            description: it.description,
            logo,
            is_like: it.likeFlag == 1,
            music_count: it.musicCount,
        }
    }
}

/// 获取歌单,不存在或已删除时返回404
async fn get_favorite(ctx: &mut DbContext, id: i64) -> Result<UserFavorite, BizError> {
    match user_favorite_dao::select_one(ctx, id).await {
        Ok(it) if it.isDelete == 0 => Ok(it),
        _ => Err(BizError::not_found()),
    }
}

/// 获取自己的歌单,不是自己的歌单时返回403
async fn get_my_favorite(ctx: &mut DbContext, id: i64, login: &LoginUser) -> Result<UserFavorite, BizError> {
    let favorite = get_favorite(ctx, id).await?;
    if favorite.userId != login.id {
        return Err(BizError::forbidden());
    }
    Ok(favorite)
}

/// 获取可以查看的歌单:自己的歌单,或者首页推荐、轮播里的歌单
async fn get_visible_favorite(ctx: &mut DbContext, id: i64, login: &LoginUser) -> Result<UserFavorite, BizError> {
    let favorite = get_favorite(ctx, id).await?;
    if favorite.userId != login.id && user_favorite_dao::count_public(ctx, id.to_string()).await? == 0 {
        return Err(BizError::forbidden());
    }
    Ok(favorite)
}

/// 我的歌单
#[GET("/api/favorite/list")]
pub async fn list(login: Ext<LoginUser>) -> Result<Vec<FavoriteVo>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = user_favorite_dao::select_by_user(&mut ctx, login.id).await?;
    Ok(list.into_iter().map(|it| FavoriteVo::new(it, login.id)).collect())
}

/// 歌单详情
#[GET("/api/favorite/detail")]
pub async fn detail(login: Ext<LoginUser>, id: i64) -> Result<FavoriteVo, BizError> {
    let mut ctx = lib_db::get_context();
    let favorite = get_visible_favorite(&mut ctx, id, &login).await?;
    Ok(FavoriteVo::new(favorite, login.id))
}

/// 新建歌单,返回歌单ID
#[POST("/api/favorite/add")]
pub async fn add(login: Ext<LoginUser>, form: form::FavoriteAdd) -> Result<i64, BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let name = form.name.trim().to_string();
    let mut ctx = db::begin().await?;
    if user_favorite_dao::count_by_name(&mut ctx, login.id, name.clone(), 0).await? > 0 {
        return Err(BizError::field("name", "已经有这个名称的歌单"));
    }
    let favorite = UserFavorite {
        name,
        date: time_util::current_millis(),
        userId: login.id,
        ..Default::default()
    };
    let id = user_favorite_dao::insert(&mut ctx, &favorite).await?;
    ctx.commit().await?;
    Ok(id)
}

/// 修改歌单名和简介
#[POST("/api/favorite/edit")]
pub async fn edit(login: Ext<LoginUser>, form: form::FavoriteEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let name = form.name.trim().to_string();
    let mut ctx = db::begin().await?;
    let favorite = get_my_favorite(&mut ctx, form.id, &login).await?;
    if favorite.likeFlag == 1 && name != favorite.name {
        return Err(BizError::field("name", "这个歌单不能改名"));
    }
    if user_favorite_dao::count_by_name(&mut ctx, login.id, name.clone(), form.id).await? > 0 {
        return Err(BizError::field("name", "已经有这个名称的歌单"));
    }
    user_favorite_dao::update_info(&mut ctx, form.id, name, non_empty(Some(form.description))).await?;
    ctx.commit().await?;
    Ok(())
}

/// 封面上传
pub struct LogoUpload {
    // 歌单ID
    pub id: i64,

    // 图片内容
    pub file: Vec<u8>,
}

/// 上传歌单封面,返回封面地址
#[POST("/api/favorite/logo")]
pub async fn logo(login: Ext<LoginUser>, form: LogoUpload) -> Result<Option<String>, BizError> {
    let mut ctx = lib_db::get_context();
    let favorite = get_my_favorite(&mut ctx, form.id, &login).await?;
    let path = upload_image(form.file).await?;
    user_favorite_dao::update_logo(&mut ctx, form.id, Some(path.clone())).await?;
    file_clean_service::remove_replaced(favorite.logo, Some(&path));
    Ok(dfs_service::file_url(Some(&path)))
}

/// 删除歌单("我喜欢"歌单不能删除)
#[DELETE("/api/favorite/delete")]
pub async fn delete(login: Ext<LoginUser>, id: i64) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    let favorite = get_my_favorite(&mut ctx, id, &login).await?;
    if favorite.likeFlag == 1 {
        return Err(biz_error!("这个歌单不能删除"));
    }
    if user_favorite_dao::count_referenced(&mut ctx, id.to_string()).await? > 0 {
        //首页的推荐分组、轮播正在使用的歌单不能真正删除,改为归管理员(所属会员ID为0),管理员可以再分配给其他会员维护
        user_favorite_dao::set_owner(&mut ctx, id, 0).await?;
        ctx.commit().await?;
    } else {
        user_favorite_dao::set_deleted(&mut ctx, id, time_util::current_millis()).await?;
        ctx.commit().await?;

        //上传的封面从DairoDFS删除
        file_clean_service::remove_replaced(favorite.logo, None);
    }
    Ok(())
}

/// 歌单排序,ids为排好序的歌单ID
#[POST("/api/favorite/sort")]
pub async fn sort(login: Ext<LoginUser>, form: form::FavoriteSort) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    for (i, it) in form.ids.split(',').filter(|it| !it.is_empty()).enumerate() {
        let Ok(id) = it.trim().parse::<i64>() else {
            return Err(BizError::field("ids", "歌单ID格式错误"));
        };

        //只会修改自己的歌单
        user_favorite_dao::update_sort(&mut ctx, id, login.id, i as i64).await?;
    }
    ctx.commit().await?;
    Ok(())
}

/// 歌单里的歌曲
#[GET("/api/favorite/musics")]
pub async fn musics(login: Ext<LoginUser>, id: i64) -> Result<Vec<MusicVo>, BizError> {
    let mut ctx = lib_db::get_context();
    get_visible_favorite(&mut ctx, id, &login).await?;
    let list = user_music_dao::select_music_by_favorite(&mut ctx, id, login.is_test as i64).await?;
    let files = music_file_dao::select_by_favorite(&mut ctx, id).await?;
    Ok(to_music_list(list, &files, vip_service::is_limited(&login)))
}

/// 我所有歌单里的歌曲(客户端同步用)
#[GET("/api/favorite/all_musics")]
pub async fn all_musics(login: Ext<LoginUser>) -> Result<Vec<MusicVo>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = user_music_dao::select_music_by_user(&mut ctx, login.id).await?;
    let files = music_file_dao::select_by_user(&mut ctx, login.id).await?;
    Ok(to_music_list(list, &files, vip_service::is_limited(&login)))
}

/// 转换成返回给客户端的歌曲列表
fn to_music_list(list: Vec<FavoriteMusicTemp>, files: &[crate::dao::music_file_dao::MusicFile], vip_limited: bool) -> Vec<MusicVo> {
    let file_map = music_service::group_files(files);
    list.into_iter()
        .map(|it| {
            MusicVo {
                id: it.id,
                name: it.name,
                artist: it.artist,
                album: it.album,
                logo: dfs_service::file_url(it.logo.as_deref()),
                duration: it.duration,
                video_state: it.videoState,
                favorite_id: Some(it.favoriteId),
                play_count: None,
                audit_state: None,
                files: music_service::files_of(&file_map, it.id, vip_limited),
                collect: music_service::collect_vo(it.platform, it.extra),
            }
        })
        .collect()
}

/// 往歌单里添加歌曲(添加到最前面)
#[POST("/api/favorite/add_music")]
pub async fn add_music(login: Ext<LoginUser>, form: form::FavoriteMusic) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    let favorite = get_my_favorite(&mut ctx, form.favorite_id, &login).await?;
    let Ok(music) = music_dao::select_one(&mut ctx, form.music_id).await else {
        return Err(biz_error!("歌曲不存在"));
    };
    if user_music_dao::count_by_favorite_music(&mut ctx, form.favorite_id, form.music_id).await? > 0 {
        return Err(biz_error!("歌单里已经有这首歌了"));
    }
    if favorite.musicCount >= MAX_MUSIC_COUNT {
        return Err(biz_error!(format!("一个歌单最多只能添加{}首歌", MAX_MUSIC_COUNT)));
    }
    let sort = user_music_dao::max_sort(&mut ctx, form.favorite_id).await? + 1;
    let user_music = UserMusic {
        favoriteId: form.favorite_id,
        musicId: form.music_id,
        userId: login.id,
        nick: None,
        sort,
    };
    user_music_dao::insert(&mut ctx, &user_music).await?;
    user_favorite_dao::refresh_music_count(&mut ctx, form.favorite_id).await?;

    //没有设置封面时,用最近添加的歌曲的封面
    let is_auto_logo = favorite.logo.as_deref().is_none_or(|it| it.starts_with(TEMP_LOGO_PREFIX));
    if is_auto_logo && let Some(logo) = music.logo {
        user_favorite_dao::update_logo(&mut ctx, form.favorite_id, Some(format!("{}{}", TEMP_LOGO_PREFIX, logo))).await?;
    }
    ctx.commit().await?;
    Ok(())
}

/// 从歌单里删除歌曲
#[DELETE("/api/favorite/remove_music")]
pub async fn remove_music(login: Ext<LoginUser>, favorite_id: i64, music_id: i64) -> Result<(), BizError> {
    let mut ctx = db::begin().await?;
    get_my_favorite(&mut ctx, favorite_id, &login).await?;
    user_music_dao::delete_by_favorite_music(&mut ctx, favorite_id, music_id).await?;
    user_favorite_dao::refresh_music_count(&mut ctx, favorite_id).await?;
    ctx.commit().await?;
    Ok(())
}
