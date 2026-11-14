//! 文件清理:把DairoDFS里不再使用的文件删除(换掉的头像、封面、轮播图,彻底删除的歌曲的文件、视频、封面等)
//! 删除的文件进DFS的回收站,到期后由DFS自动清理

use crate::dao::music_dao;
use crate::service::dfs_service;

/// 歌单自动使用歌曲封面时,封面保存为 temp:存储路径
pub const TEMP_LOGO_PREFIX: &str = "temp:";

/// 图片换掉之后调用(要在数据库更新之后):旧图片没有别的地方使用时,从DairoDFS删除
/// 在后台执行,删除失败只记录日志,不影响换图
/// - old 换掉之前的图片
/// - new 换上的图片,没有时为None
pub fn remove_replaced(old: Option<String>, new: Option<&str>) {
    if let Some(old) = old.filter(|it| new != Some(it.as_str())) {
        remove_unused_later(vec![old]);
    }
}

/// 在后台删除不再使用的文件
pub fn remove_unused_later(paths: Vec<String>) {
    if !paths.is_empty() {
        tokio::spawn(remove_unused(paths));
    }
}

/// 数据库里已经没有地方使用的文件,从DairoDFS删除(要在数据库更新、提交之后调用)
/// 只处理DairoDFS里的文件:采集平台的网址、歌单自动使用的歌曲封面(temp:开头)都不处理
/// 删除失败只记录日志
pub async fn remove_unused(paths: Vec<String>) {
    let mut ctx = lib_db::get_context();
    for path in paths.into_iter().filter(|it| it.starts_with('/')) {
        let temp_path = format!("{}{}", TEMP_LOGO_PREFIX, path);
        match music_dao::count_path_used(&mut ctx, path.clone(), temp_path).await {
            Ok(0) => {}
            Ok(_) => continue,
            Err(e) => {
                eprintln!("-->检查文件{}是否还在使用失败: {}", path, e);
                continue;
            }
        }
        if let Err(e) = dfs_service::delete(&path).await {
            eprintln!("-->删除文件{}失败: {}", path, e);
        }
    }
}
