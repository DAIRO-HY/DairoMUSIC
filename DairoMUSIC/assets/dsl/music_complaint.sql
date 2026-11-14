-- 后台的投诉列表(分页),带上歌曲当前的信息、这首歌被投诉的次数
-- @name page_list
-- @page
-- @param state:i64?
-- @param music_id:i64?
-- @param user_id:i64?
select c.id, c.musicId, c.content, c.state, c.userId, c.adminId, c.date, c.handleTime,
       (select m.name from music m where m.id = c.musicId) as musicName,
       (select m.artist from music m where m.id = c.musicId) as musicArtist,
       (select m.auditState from music m where m.id = c.musicId) as musicAuditState,
       (select m.deleteFlag from music m where m.id = c.musicId) as musicDeleteFlag,
       (select count(*) from music_complaint t where t.musicId = c.musicId) as complaintCount,
       (select u.email from user u where u.id = c.userId) as userEmail,
       (select a.name from admin a where a.id = c.adminId) as adminName
from music_complaint c
where 1 = 1
-- @if state != null
and c.state = :state
-- @end
-- @if music_id != null
and c.musicId = :music_id
-- @end
-- @if user_id != null
and c.userId = :user_id
-- @end

-- ------------------------------

-- 投诉的歌曲ID
-- @name select_music_ids
-- @param ids:List<i64>
-- @return List<i64>
select distinct musicId from music_complaint where 1 = 1
-- @each ids seq="," open="and id in (" close=")" item="id"
:id
-- @end

-- ------------------------------

-- 标记为已处理(只处理还没有处理的)
-- @name set_handled
-- @param ids:List<i64>
-- @param admin_id:i64
-- @param handle_time:i64
update music_complaint set state = 1, adminId = :admin_id, handleTime = :handle_time where state = 0
-- @each ids seq="," open="and id in (" close=")" item="id"
:id
-- @end

-- ------------------------------

-- 某些歌曲还没有处理的投诉全部标记为已处理(歌曲被拒绝之后,同一首歌的其他投诉不用再处理)
-- @name set_handled_by_musics
-- @param music_ids:List<i64>
-- @param admin_id:i64
-- @param handle_time:i64
update music_complaint set state = 1, adminId = :admin_id, handleTime = :handle_time where state = 0
-- @each music_ids seq="," open="and musicId in (" close=")" item="id"
:id
-- @end

-- ------------------------------

-- 还没有处理的投诉数量
-- @name count_pending
-- @return i64
select count(*) from music_complaint where state = 0;


-- ------------------------------

-- 删除歌曲的投诉(彻底删除歌曲时)
-- @name delete_by_music
-- @param music_id:i64
delete from music_complaint where musicId = :music_id;
