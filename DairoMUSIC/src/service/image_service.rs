//! 图片清理:头像、封面、轮播图换掉之后,把DairoDFS里不再使用的旧图片删除

use crate::dao::music_dao;
use crate::service::dfs_service;

/// 歌单自动使用歌曲封面时,封面保存为 temp:存储路径
pub const TEMP_LOGO_PREFIX: &str = "temp:";

/// 图片换掉之后调用(要在数据库更新之后):旧图片没有别的地方使用时,从DairoDFS删除
/// 在后台执行,删除失败只记录日志,不影响换图
/// - old 换掉之前的图片
/// - new 换上的图片
pub fn remove_replaced(old: Option<String>, new: Option<&str>) {
    let Some(old) = old else {
        return;
    };

    //只删除DairoDFS里的图片:采集平台的网址、歌单自动使用的歌曲封面(temp:开头)都不处理
    if !old.starts_with("/image/") || new == Some(old.as_str()) {
        return;
    }
    tokio::spawn(async move {
        let mut ctx = lib_db::get_context();
        let temp_path = format!("{}{}", TEMP_LOGO_PREFIX, old);
        match music_dao::count_image_used(&mut ctx, old.clone(), temp_path).await {
            Ok(0) => {}
            Ok(_) => return,
            Err(e) => {
                eprintln!("-->检查图片是否还在使用失败: {}", e);
                return;
            }
        }
        if let Err(e) = dfs_service::delete(&old).await {
            eprintln!("-->删除旧图片{}失败: {}", old, e);
        }
    });
}
