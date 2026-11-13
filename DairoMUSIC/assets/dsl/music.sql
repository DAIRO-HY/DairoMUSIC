-- 歌曲列表(分页)
-- @name page_list
-- @page
-- @param id:i64?
-- @param name:String? 歌名(模糊匹配)
-- @param artist:String? 歌手(模糊匹配)
-- @param album:String? 专辑(模糊匹配)
-- @param state:i64?
-- @param audit_state:i64?
-- @param delete_flag:i64?
-- @return Music
select * from music where 1 = 1
-- @if id != null
and id = :id
-- @end
-- @if name != null
and name like '%' || :name || '%'
-- @end
-- @if artist != null
and artist like '%' || :artist || '%'
-- @end
-- @if album != null
and album like '%' || :album || '%'
-- @end
-- @if state != null
and state = :state
-- @end
-- @if audit_state != null
and auditState = :audit_state
-- @end
-- @if delete_flag != null
and deleteFlag = :delete_flag
-- @end

-- ------------------------------

-- 后台更新歌曲信息
-- @name update_by_admin
-- @param id:i64
-- @param name:String?
-- @param artist:String?
-- @param album:String?
-- @param state:i64
-- @param audit_state:i64
update music set name = :name, artist = :artist, album = :album, state = :state, auditState = :audit_state where id = :id;

-- ------------------------------

-- 批量修改审核状态
-- @name set_audit_state
-- @param ids:List<i64>
-- @param audit_state:i64
update music set auditState = :audit_state where 1 = 1
-- @each ids seq="," open="and id in (" close=")" item="id"
:id
-- @end

-- ------------------------------

-- 标记为删除
-- @name set_deleted
-- @param id:i64
-- @param delete_time:i64
update music set deleteFlag = 1, deleteTime = :delete_time where id = :id;

-- ------------------------------

-- 恢复已删除的歌曲
-- @name set_restored
-- @param id:i64
update music set deleteFlag = 0, deleteTime = null where id = :id;

-- ------------------------------

-- 更新封面
-- @name update_logo
-- @param id:i64
-- @param logo:String?
update music set logo = :logo where id = :id;

-- ------------------------------

-- 重复的歌曲(歌名和歌手都相同)
-- @name page_duplicate
-- @page
-- @param name:String? 歌名(模糊匹配)
-- @param artist:String? 歌手(模糊匹配)
select name, artist, count(*) as count from music where deleteFlag = 0
-- @if name != null
and name like '%' || :name || '%'
-- @end
-- @if artist != null
and artist like '%' || :artist || '%'
-- @end
group by name, artist having count(*) > 1

-- ------------------------------

-- 重复的歌曲(只看歌名,忽略歌手)
-- @name page_duplicate_ignore_artist
-- @page
-- @param name:String? 歌名(模糊匹配)
select name, count(*) as count from music where deleteFlag = 0
-- @if name != null
and name like '%' || :name || '%'
-- @end
group by name having count(*) > 1

-- ------------------------------

-- 歌名(和歌手)相同的未删除歌曲,带上最高音质文件的大小和格式
-- @name select_same
-- @param name:String
-- @param artist:String?
-- @param ignore_artist:i64 1:忽略歌手
-- @return List<SelectSameTemp>
select m.id, m.name, m.artist, m.album, m.logo, m.duration, m.state, m.auditState, m.maxBitRate, m.playCount,
       (select f.size from music_file f where f.musicId = m.id order by f.bitRate desc limit 1) as size,
       (select f.format from music_file f where f.musicId = m.id order by f.bitRate desc limit 1) as format
from music m
where m.deleteFlag = 0 and m.name = :name
  and (:ignore_artist = 1 or m.artist is :artist)
order by m.maxBitRate desc, m.id;

-- ------------------------------

-- 歌曲详情(不包括已删除的歌曲)
-- @name select_detail
-- @param id:i64
-- @return MusicDetailTemp?
select m.id, m.name, m.artist, m.album, m.logo, m.duration, m.maxBitRate, m.state, m.videoState, c.platform, c.extra
from music m
         left join music_collect c on c.musicId = m.id
where m.id = :id and m.deleteFlag = 0;

-- ------------------------------

-- 搜索歌曲(歌名或歌手包含任意一个关键词):只搜索采集完成并且没有被拒绝的歌曲,审核通过的排在前面
-- @name search
-- @param keys:List<String>
-- @param is_test:i64 1:只搜索测试歌曲
-- @return List<Music>
select * from music where deleteFlag = 0 and state = 100 and auditState != 2 and (:is_test = 0 or testFlag = 1)
-- @each keys seq=" or " open="and (" close=")" item="key"
name like '%' || :key || '%' or artist like '%' || :key || '%'
-- @end
order by auditState desc limit 2000

-- ------------------------------

-- 纠错:更新歌名、歌手、专辑(不改变审核状态)
-- @name correct
-- @param id:i64
-- @param name:String?
-- @param artist:String?
-- @param album:String?
update music set name = :name, artist = :artist, album = :album where id = :id;

-- ------------------------------

-- 采集完成,更新歌曲信息,采集状态改为完成(审核状态不变,新歌曲是待审核)
-- @name update_collected
-- @param id:i64
-- @param name:String?
-- @param artist:String?
-- @param album:String?
-- @param logo:String?
-- @param duration:i64
-- @param year:i64?
-- @param max_bit_rate:i64
update music set name = :name, artist = :artist, album = :album, logo = :logo, duration = :duration,
                 year = :year, maxBitRate = :max_bit_rate, state = 100
where id = :id;

-- ------------------------------

-- 采集失败(只修改还在采集中的歌曲)
-- @name set_collect_fail
-- @param id:i64
update music set state = 1 where id = :id and state = 0;

-- ------------------------------

-- 更新最高音质
-- @name update_max_bit_rate
-- @param id:i64
-- @param max_bit_rate:i64
update music set maxBitRate = :max_bit_rate where id = :id;

-- ------------------------------

-- 重新采集时状态改回采集中
-- @name set_collecting
-- @param id:i64
update music set state = 0 where id = :id;

-- ------------------------------

-- 修改视频状态
-- @name set_video_state
-- @param id:i64
-- @param video_state:i64
-- @param now:i64
update music set videoState = :video_state, videoTime = :now where id = :id;

-- ------------------------------

-- 请求采集视频:还没有采集过,或者采集失败超过一段时间的才改为采集中,返回修改的行数
-- @name request_video
-- @param id:i64
-- @param now:i64
-- @param retry_before:i64 采集失败的在这个时间之前失败的才重试
update music set videoState = 1, videoTime = :now
where id = :id and deleteFlag = 0 and (videoState = 0 or (videoState = 5 and videoTime < :retry_before));

-- ------------------------------

-- 下一个要采集视频的歌曲(最早请求的先采集)
-- @name select_video_next
-- @param platform:String
-- @return VideoNextTemp?
select m.id, c.extra
from music m
         join music_collect c on c.musicId = m.id
where m.videoState = 1 and m.deleteFlag = 0 and c.platform = :platform
order by m.videoTime
limit 1;

-- ------------------------------

-- 不是采集平台的歌曲不能采集视频,改为没有视频
-- @name set_video_none_not_platform
-- @param platform:String
-- @param now:i64
update music set videoState = 3, videoTime = :now
where videoState = 1 and id not in (select musicId from music_collect where platform = :platform);
