-- 歌曲的所有文件,音质从低到高
-- @name select_by_music
-- @param music_id:i64
-- @return List<MusicFile>
select * from music_file where musicId = :music_id order by bitRate;

-- ------------------------------

-- 通过MD5获取歌曲文件
-- @name select_by_md5
-- @param md5:String
-- @return MusicFile?
select * from music_file where md5 = :md5;

-- ------------------------------

-- 歌单里所有歌曲的文件
-- @name select_by_favorite
-- @param favorite_id:i64
-- @return List<MusicFile>
select * from music_file where musicId in (select musicId from user_music where favoriteId = :favorite_id);

-- ------------------------------

-- 会员所有歌单里的歌曲的文件
-- @name select_by_user
-- @param user_id:i64
-- @return List<MusicFile>
select * from music_file where musicId in (select musicId from user_music where userId = :user_id);

-- ------------------------------

-- 多首歌曲的文件
-- @name select_by_musics
-- @param music_ids:List<i64>
-- @return List<MusicFile>
select * from music_file where 1 = 1
-- @each music_ids seq="," open="and musicId in (" close=")" item="id"
:id
-- @end
