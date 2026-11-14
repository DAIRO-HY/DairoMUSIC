//! 首页(发现页)的轮播图

use crate::dao::swiper_dao::Swiper;
use crate::dao::{music_dao, swiper_dao, user_favorite_dao};
use crate::service::dfs_service;
use crate::util::{ffmpeg_util, image_util, time_util};
use crate::web::biz_error::BizError;
use crate::web::form;
use crate::{biz_error, db};
use hyperoute_macro::{GET, POST};
use rand::distr::{Alphanumeric, SampleString};
use serde::Serialize;
use sqlx_context::DbContext;
use validator::Validate;

/// 类型 1:推荐歌单 2:推荐单曲 3:拨打电话 4:跳转网页
const TYPE_FAVORITE: i64 = 1;
const TYPE_MUSIC: i64 = 2;
const TYPE_PHONE: i64 = 3;
const TYPE_URL: i64 = 4;

/// 轮播
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SwiperItem {
    pub id: i64,
    pub name: String,

    // 图片的存储路径
    pub image: String,

    // 图片的访问地址
    pub image_url: Option<String>,

    // 类型 1:推荐歌单 2:推荐单曲 3:拨打电话 4:跳转网页
    pub swiper_type: i64,

    // 数据:歌单ID、歌曲ID、电话号码、网址
    pub data: Option<String>,

    // 数据的显示文字(歌单名、歌名),歌单、歌曲已经不存在时为空
    pub data_text: Option<String>,
    pub state: i64,
    pub sort: i64,
}

/// 歌单名、歌名;不是歌单和歌曲时就是数据本身
async fn data_text(ctx: &mut DbContext, swiper_type: i64, data: Option<&str>) -> Option<String> {
    let data = data?;
    match swiper_type {
        TYPE_FAVORITE => match user_favorite_dao::select_one(ctx, data.parse().ok()?).await {
            Ok(it) if it.isDelete == 0 => Some(it.name),
            _ => None,
        },
        TYPE_MUSIC => match music_dao::select_one(ctx, data.parse().ok()?).await {
            Ok(it) if it.deleteFlag == 0 => Some(it.name.unwrap_or_default()),
            _ => None,
        },
        _ => Some(data.to_string()),
    }
}

async fn to_item(ctx: &mut DbContext, it: Swiper) -> SwiperItem {
    SwiperItem {
        data_text: data_text(ctx, it.swiperType, it.data.as_deref()).await,
        image_url: dfs_service::file_url(Some(&it.image)),
        id: it.id,
        name: it.name,
        image: it.image,
        swiper_type: it.swiperType,
        data: it.data,
        state: it.state,
        sort: it.sort,
    }
}

/// 轮播列表
#[GET("/admin/api/swiper/list")]
pub async fn list() -> Result<Vec<SwiperItem>, BizError> {
    let mut ctx = lib_db::get_context();
    let mut list = Vec::new();
    for it in swiper_dao::select_list(&mut ctx).await? {
        list.push(to_item(&mut ctx, it).await);
    }
    Ok(list)
}

/// 轮播详情,id为0时返回新增用的默认值
#[GET("/admin/api/swiper/detail")]
pub async fn detail(id: i64) -> Result<SwiperItem, BizError> {
    if id == 0 {
        return Ok(SwiperItem {
            swiper_type: TYPE_FAVORITE,
            state: 1,
            ..Default::default()
        });
    }
    let mut ctx = lib_db::get_context();
    let Ok(swiper) = swiper_dao::select_one(&mut ctx, id).await else {
        return Err(biz_error!("轮播不存在"));
    };
    Ok(to_item(&mut ctx, swiper).await)
}

/// 图片上传
pub struct ImageUpload {
    // 图片内容
    pub file: Vec<u8>,
}

/// 上传的图片
#[derive(Serialize)]
pub struct ImageResult {
    // 存储路径(保存轮播时提交)
    pub path: String,

    // 访问地址
    pub url: Option<String>,
}

/// 轮播图的分辨率,按APP的显示标准:宽高比2.4:1(APP里居中裁切显示),最大的设备上显示为1536x640
const IMAGE_WIDTH: u32 = 1920;
const IMAGE_HEIGHT: u32 = 800;

/// 上传轮播图片(保存到DairoDFS),保存轮播时提交返回的存储路径
/// 不限制上传的图片大小,上传后居中裁切成2.4:1并缩小到1920x800,转换成JPG
#[POST("/admin/api/swiper/upload_image")]
pub async fn upload(form: ImageUpload) -> Result<ImageResult, BizError> {
    if image_util::image_ext(&form.file).is_none() {
        return Err(biz_error!("只支持JPG、PNG、WEBP格式的图片"));
    }

    let name = Alphanumeric.sample_string(&mut rand::rng(), 16);
    let data = match ffmpeg_util::crop_image_data(form.file, IMAGE_WIDTH, IMAGE_HEIGHT).await {
        Ok(it) => it,
        Err(e) => {
            eprintln!("-->轮播图片处理失败: {}", e);
            return Err(biz_error!("图片处理失败,请检查图片是否损坏,以及「依赖软件」里的ffmpeg是否可以使用"));
        }
    };
    let path = format!("/image/swiper/{}/{}.jpg", time_util::current_year(), name);
    if let Err(e) = dfs_service::upload(data, &path, "").await {
        return Err(biz_error!(format!("图片上传失败: {}", e)));
    }
    Ok(ImageResult {
        url: dfs_service::file_url(Some(&path)),
        path,
    })
}

/// 添加或修改轮播
#[POST("/admin/api/swiper/edit")]
pub async fn edit(form: form::SwiperEdit) -> Result<(), BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    if ![0, 1].contains(&form.state) {
        return Err(BizError::field("state", "状态不正确"));
    }

    //图片只能是上传接口返回的路径
    if !form.image.starts_with("/image/") || form.image.contains("..") {
        return Err(BizError::field("image", "请上传图片"));
    }
    let data = form.data.trim().to_string();
    let mut ctx = db::begin().await?;
    match form.swiper_type {
        TYPE_FAVORITE => {
            let favorite = match data.parse::<i64>() {
                Ok(id) => user_favorite_dao::select_one(&mut ctx, id).await.ok(),
                Err(_) => None,
            };
            if !favorite.is_some_and(|it| it.isDelete == 0 && it.likeFlag == 0) {
                return Err(BizError::field("data", "请选择歌单"));
            }
        }
        TYPE_MUSIC => {
            let music = match data.parse::<i64>() {
                Ok(id) => music_dao::select_one(&mut ctx, id).await.ok(),
                Err(_) => None,
            };
            if !music.is_some_and(|it| it.deleteFlag == 0) {
                return Err(BizError::field("data", "请选择歌曲"));
            }
        }
        TYPE_PHONE => {
            if data.is_empty() || !data.chars().all(|it| it.is_ascii_digit() || "+- ".contains(it)) {
                return Err(BizError::field("data", "请输入电话号码"));
            }
        }
        TYPE_URL => {
            if !data.starts_with("http://") && !data.starts_with("https://") {
                return Err(BizError::field("data", "请输入http://或https://开头的网址"));
            }
        }
        _ => return Err(BizError::field("swiperType", "类型不正确")),
    }
    let mut old_image = None;
    let mut new_image = String::new();
    if form.id == 0 {
        let swiper = Swiper {
            name: form.name.trim().to_string(),
            image: form.image,
            data: Some(data),
            swiperType: form.swiper_type,
            state: form.state,
            sort: form.sort,
            date: time_util::current_millis(),
            ..Default::default()
        };
        swiper_dao::insert(&mut ctx, &swiper).await?;
    } else {
        let Ok(mut swiper) = swiper_dao::select_one(&mut ctx, form.id).await else {
            return Err(biz_error!("轮播不存在"));
        };
        old_image = Some(std::mem::replace(&mut swiper.image, form.image));
        swiper.name = form.name.trim().to_string();
        swiper.data = Some(data);
        swiper.swiperType = form.swiper_type;
        swiper.state = form.state;
        swiper.sort = form.sort;
        new_image = swiper.image.clone();
        swiper_dao::update(&mut ctx, &swiper).await?;
    }
    ctx.commit().await?;

    //换了图片时,旧图片从DairoDFS删除
    crate::service::image_service::remove_replaced(old_image, Some(&new_image));
    Ok(())
}

/// 删除轮播
#[POST("/admin/api/swiper/delete")]
pub async fn delete(id: i64) -> Result<(), BizError> {
    let mut ctx = lib_db::get_context();
    swiper_dao::delete(&mut ctx, id).await?;
    Ok(())
}
