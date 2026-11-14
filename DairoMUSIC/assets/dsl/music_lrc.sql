-- 歌曲的歌词
-- @name select_by_music
-- @param music_id:i64
-- @return List<MusicLrc>
select * from music_lrc where musicId = :music_id order by state desc, agreeCount desc, id;


-- ------------------------------

-- 删除歌曲的歌词(彻底删除歌曲时)
-- @name delete_by_music
-- @param music_id:i64
delete from music_lrc where musicId = :music_id;
