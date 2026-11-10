-- 歌曲的视频
-- @name select_by_music
-- @param music_id:i64
-- @return List<MusicVideo>
select * from music_video where musicId = :music_id order by quality;

-- ------------------------------

-- 删除歌曲的视频
-- @name delete_by_music
-- @param music_id:i64
delete from music_video where musicId = :music_id;

-- ------------------------------

-- 处理完成(转码、上传成功)
-- @name set_done
-- @param id:i64
-- @param codec:String
-- @param width:i64
-- @param height:i64
-- @param bit_rate:i64
-- @param frame_rate:i64
-- @param duration:i64
-- @param size:i64
-- @param md5:String
-- @param path:String
-- @param memo:String
update music_video set state = 1, codec = :codec, width = :width, height = :height, bitRate = :bit_rate, frameRate = :frame_rate,
                       duration = :duration, size = :size, md5 = :md5, path = :path, memo = :memo
where id = :id;

-- ------------------------------

-- 处理失败
-- @name set_fail
-- @param id:i64
-- @param memo:String
update music_video set state = 2, memo = :memo where id = :id;

-- ------------------------------

-- 还在处理中的视频(程序被关闭时没有处理完)
-- @name select_processing_music
-- @return List<i64>
select distinct musicId from music_video where state = 0;
