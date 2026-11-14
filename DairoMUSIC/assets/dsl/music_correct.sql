-- 后台的纠错列表(分页),带上歌曲当前的信息
-- @name page_list
-- @page
-- @param state:i64?
-- @param music_id:i64?
-- @param user_id:i64?
select c.id, c.musicId, c.data, c.state, c.userId, c.adminId, c.date, c.handleTime,
       (select m.name from music m where m.id = c.musicId) as musicName,
       (select m.artist from music m where m.id = c.musicId) as musicArtist,
       (select m.album from music m where m.id = c.musicId) as musicAlbum,
       (select m.deleteFlag from music m where m.id = c.musicId) as musicDeleteFlag,
       (select u.email from user u where u.id = c.userId) as userEmail,
       (select a.name from admin a where a.id = c.adminId) as adminName
from music_correct c
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

-- 还没有处理的纠错
-- @name select_pendings
-- @param ids:List<i64>
-- @return List<MusicCorrect>
select * from music_correct where state = 0
-- @each ids seq="," open="and id in (" close=")" item="id"
:id
-- @end
order by id;

-- ------------------------------

-- 处理纠错(只处理还没有处理的)
-- @name set_state
-- @param id:i64
-- @param state:i64 1:已同意 2:已拒绝
-- @param admin_id:i64
-- @param handle_time:i64
update music_correct set state = :state, adminId = :admin_id, handleTime = :handle_time where id = :id and state = 0;

-- ------------------------------

-- 还没有处理的纠错数量
-- @name count_pending
-- @return i64
select count(*) from music_correct where state = 0;
