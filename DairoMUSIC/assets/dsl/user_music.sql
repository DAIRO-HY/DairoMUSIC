-- 收藏了某首歌曲的记录
-- @name select_by_music
-- @param music_id:i64
-- @return List<UserMusic>
select * from user_music where musicId = :music_id;

-- ------------------------------

-- 收藏里是否已经有这首歌
-- @name count_by_favorite_music
-- @param favorite_id:i64
-- @param music_id:i64
-- @return i64
select count(*) from user_music where favoriteId = :favorite_id and musicId = :music_id;

-- ------------------------------

-- 把收藏里的一首歌换成另一首
-- @name update_music_id
-- @param favorite_id:i64
-- @param music_id:i64
-- @param new_music_id:i64
update user_music set musicId = :new_music_id where favoriteId = :favorite_id and musicId = :music_id;

-- ------------------------------

-- 从收藏里删除一首歌
-- @name delete_by_favorite_music
-- @param favorite_id:i64
-- @param music_id:i64
delete from user_music where favoriteId = :favorite_id and musicId = :music_id;

-- ------------------------------

-- 歌单里的歌曲(不包括已删除的歌曲)
-- @name select_music_by_favorite
-- @param favorite_id:i64
-- @param is_test:i64 1:只返回测试歌曲
-- @return List<FavoriteMusicTemp>
select m.id, m.name, m.artist, m.album, m.logo, m.duration, m.maxBitRate, m.collectState, m.videoState, um.favoriteId, c.platform, c.extra
from user_music um
         inner join music m on um.musicId = m.id
         left join music_collect c on c.musicId = m.id
where um.favoriteId = :favorite_id and m.deleteFlag = 0 and (:is_test = 0 or m.testFlag = 1)
order by um.sort desc;

-- ------------------------------

-- 会员所有歌单里的歌曲
-- @name select_music_by_user
-- @param user_id:i64
-- @return List<FavoriteMusicTemp>
select m.id, m.name, m.artist, m.album, m.logo, m.duration, m.maxBitRate, m.collectState, m.videoState, um.favoriteId, c.platform, c.extra
from user_music um
         inner join user_favorite uf on uf.id = um.favoriteId
         inner join music m on um.musicId = m.id
         left join music_collect c on c.musicId = m.id
where um.userId = :user_id and uf.isDelete = 0 and m.deleteFlag = 0
order by um.favoriteId, um.sort desc;

-- ------------------------------

-- 歌单里最大的排序号
-- @name max_sort
-- @param favorite_id:i64
-- @return i64
select coalesce(max(sort), 0) from user_music where favoriteId = :favorite_id;


-- ------------------------------

-- 删除歌曲的歌单里的收藏(彻底删除歌曲时)
-- @name delete_by_music
-- @param music_id:i64
delete from user_music where musicId = :music_id;

-- ------------------------------

-- 删除歌单里的所有歌曲(后台保存歌单时按新的顺序重新添加)
-- @name delete_by_favorite
-- @param favorite_id:i64
delete from user_music where favoriteId = :favorite_id;

-- ------------------------------

-- 修改歌单里的歌曲所属的会员(歌单换了所属的会员时)
-- @name update_user_by_favorite
-- @param favorite_id:i64
-- @param user_id:i64
update user_music set userId = :user_id where favoriteId = :favorite_id;
