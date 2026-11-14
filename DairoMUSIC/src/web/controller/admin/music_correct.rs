//! 歌曲纠错:查看、处理APP提交的纠错

use crate::dao::music_correct_dao::{self, PageListTemp};
use crate::dao::music_dao;
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::filter::auth::LoginAdmin;
use crate::web::model::{parse_ids, PageData, PageForm};
use crate::{biz_errorf, db};
use hyperoute_core::{Ext, Query};
use hyperoute_macro::{GET, POST};
use serde::Deserialize;

/// 处理结果:已同意
const STATE_AGREE: i64 = 1;

/// 处理结果:已拒绝
const STATE_REFUSE: i64 = 2;

/// 纠错内容里的一项(music_correct.data是这个结构的数组)
#[derive(Deserialize)]
struct CorrectItem {
    // 字段 name:歌名 artist:歌手 album:专辑
    field: String,

    // 修改之后的值
    after: String,
}

/// 纠错列表,带上歌曲当前的歌名、歌手、专辑
/// - state 0:未处理 1:已同意 2:已拒绝
#[GET("/admin/api/music_correct/list")]
pub async fn list(page: Query<PageForm>, state: Option<i64>, music_id: Option<i64>, user_id: Option<i64>) -> Result<PageData<PageListTemp>, BizError> {
    let page_in = page.to_page_in(&["id", "musicId", "state", "userId", "date", "handleTime"], "id", "desc");
    let mut ctx = lib_db::get_context();
    let (total, list) = music_correct_dao::page_list(&mut ctx, page_in, state, music_id, user_id).await?;
    Ok(PageData::new(total, list))
}

/// 同意纠错:把纠错内容写入歌曲(歌曲在提交纠错之后被修改过时,仍然按纠错内容写入)
/// - ids 纠错ID,逗号分隔;已经处理过的会跳过
#[POST("/admin/api/music_correct/agree")]
pub async fn agree(login: Ext<LoginAdmin>, ids: String) -> Result<(), BizError> {
    let ids = parse_ids(&ids, "纠错", 500)?;
    let now = time_util::current_millis();
    let mut ctx = db::begin().await?;

    //按提交的先后顺序处理,同一首歌有多条纠错时以后提交的为准
    for correct in music_correct_dao::select_pendings(&mut ctx, ids).await? {
        let Ok(music) = music_dao::select_one(&mut ctx, correct.musicId).await else {
            return Err(biz_errorf!("纠错ID:{}的歌曲已经不存在,请拒绝这条纠错", correct.id));
        };
        let Ok(items) = serde_json::from_str::<Vec<CorrectItem>>(&correct.data) else {
            return Err(biz_errorf!("纠错ID:{}的内容格式不正确,请拒绝这条纠错", correct.id));
        };
        let (mut name, mut artist, mut album) = (music.name, music.artist, music.album);
        for it in items {
            match it.field.as_str() {
                "name" => name = Some(it.after),
                "artist" => artist = Some(it.after),
                "album" => album = Some(it.after),
                _ => {}
            }
        }
        music_dao::correct(&mut ctx, correct.musicId, name, artist, album).await?;
        music_correct_dao::set_state(&mut ctx, correct.id, STATE_AGREE, login.id, now).await?;
    }
    ctx.commit().await?;
    Ok(())
}

/// 拒绝纠错
/// - ids 纠错ID,逗号分隔;已经处理过的会跳过
#[POST("/admin/api/music_correct/refuse")]
pub async fn refuse(login: Ext<LoginAdmin>, ids: String) -> Result<(), BizError> {
    let ids = parse_ids(&ids, "纠错", 500)?;
    let now = time_util::current_millis();
    let mut ctx = db::begin().await?;
    for id in ids {
        music_correct_dao::set_state(&mut ctx, id, STATE_REFUSE, login.id, now).await?;
    }
    ctx.commit().await?;
    Ok(())
}
