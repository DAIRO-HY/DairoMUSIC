//! 歌曲投诉:查看、处理APP提交的投诉

use crate::dao::music_complaint_dao::{self, PageListTemp};
use crate::dao::music_dao;
use crate::db;
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::filter::auth::LoginAdmin;
use crate::web::model::{parse_ids, PageData, PageForm};
use hyperoute_core::{Ext, Query};
use hyperoute_macro::{GET, POST};

/// 歌曲的审核状态:拒绝(APP搜索不显示)
const AUDIT_REFUSE: i64 = 2;

/// 投诉列表,带上歌曲当前的歌名、歌手、审核状态,以及这首歌被投诉的次数
/// - state 0:未处理 1:已处理
#[GET("/admin/api/music_complaint/list")]
pub async fn list(page: Query<PageForm>, state: Option<i64>, music_id: Option<i64>, user_id: Option<i64>) -> Result<PageData<PageListTemp>, BizError> {
    let page_in = page.to_page_in(&["id", "musicId", "state", "userId", "date", "handleTime"], "id", "desc");
    let mut ctx = lib_db::get_context();
    let (total, list) = music_complaint_dao::page_list(&mut ctx, page_in, state, music_id, user_id).await?;
    Ok(PageData::new(total, list))
}

/// 标记为已处理
/// - ids 投诉ID,逗号分隔;已经处理过的会跳过
#[POST("/admin/api/music_complaint/handle")]
pub async fn handle(login: Ext<LoginAdmin>, ids: String) -> Result<(), BizError> {
    let ids = parse_ids(&ids, "投诉", 1000)?;
    let mut ctx = lib_db::get_context();
    music_complaint_dao::set_handled(&mut ctx, ids, login.id, time_util::current_millis()).await?;
    Ok(())
}

/// 把被投诉的歌曲的审核状态改成拒绝(APP搜索不显示),这些歌曲还没有处理的投诉全部标记为已处理
/// - ids 投诉ID,逗号分隔
#[POST("/admin/api/music_complaint/refuse_music")]
pub async fn refuse_music(login: Ext<LoginAdmin>, ids: String) -> Result<(), BizError> {
    let ids = parse_ids(&ids, "投诉", 1000)?;
    let mut ctx = db::begin().await?;
    let music_ids = music_complaint_dao::select_music_ids(&mut ctx, ids).await?;

    //没有ID时批量更新的语句会更新所有歌曲,必须检查
    if music_ids.is_empty() {
        return Err(BizError::not_found());
    }
    music_dao::set_audit_state(&mut ctx, music_ids.clone(), AUDIT_REFUSE).await?;
    music_complaint_dao::set_handled_by_musics(&mut ctx, music_ids, login.id, time_util::current_millis()).await?;
    ctx.commit().await?;
    Ok(())
}
