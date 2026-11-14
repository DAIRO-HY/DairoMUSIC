-- 通过采集参数获取采集记录
-- @name select_by_extra
-- @param extra:String
-- @return MusicCollect?
select * from music_collect where extra = :extra;

-- ------------------------------

-- 需要处理的采集任务:未处理的,以及失败后过了一段时间、次数没有超过上限的
-- @name select_need
-- @param retry_before:i64 失败的任务在这个时间之前更新的才重试
-- @param max_count:i64 最多处理的次数
-- @param platforms:List<String> 要处理的采集平台
-- @param limit:i64
-- @return List<MusicCollect>
select * from music_collect
where downloadCount < :max_count and (state = -1 or (state = 2 and updateTime < :retry_before))
-- @each platforms seq="," open="and platform in (" close=")" item="it"
:it
-- @end
order by downloadCount, id limit :limit

-- ------------------------------

-- 标记为处理中,同时处理次数加1(只有未处理和失败的任务能标记成功,返回修改的行数)
-- @name set_collecting
-- @param id:i64
-- @param now:i64
update music_collect set state = 0, downloadCount = downloadCount + 1, memo = '准备中', updateTime = :now
where id = :id and state in (-1, 2);

-- ------------------------------

-- 更新处理进度
-- @name set_memo
-- @param id:i64
-- @param memo:String
-- @param now:i64
update music_collect set memo = :memo, updateTime = :now where id = :id;

-- ------------------------------

-- 更新处理结果 state 1:成功 2:失败
-- @name set_result
-- @param id:i64
-- @param state:i64
-- @param memo:String
-- @param now:i64
update music_collect set state = :state, memo = :memo, updateTime = :now where id = :id;

-- ------------------------------

-- 程序启动时,上次没有处理完的任务改为未处理
-- @name reset_collecting
update music_collect set state = -1 where state = 0;

-- ------------------------------

-- 更新下载地址
-- @name set_url
-- @param id:i64
-- @param url:String
update music_collect set url = :url where id = :id;

-- ------------------------------

-- 歌曲的采集记录
-- @name select_by_music
-- @param music_id:i64
-- @return MusicCollect?
select * from music_collect where musicId = :music_id;

-- ------------------------------

-- 重新处理(后台手动操作):清空处理次数,改为未处理
-- @name reset
-- @param id:i64
-- @param now:i64
update music_collect set state = -1, downloadCount = 0, memo = '等待重新处理', updateTime = :now where id = :id and state != 0;


-- ------------------------------

-- 删除歌曲的采集记录(彻底删除歌曲时)
-- @name delete_by_music
-- @param music_id:i64
delete from music_collect where musicId = :music_id;
