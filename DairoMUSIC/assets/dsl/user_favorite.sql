-- 按收藏里实际的歌曲数更新歌曲数
-- @name refresh_music_count
-- @param id:i64
update user_favorite set musicCount = (select count(*) from user_music where favoriteId = :id) where id = :id;

-- ------------------------------

-- 会员的所有歌单(未删除)
-- @name select_by_user
-- @param user_id:i64
-- @return List<UserFavorite>
select * from user_favorite where userId = :user_id and isDelete = 0 order by likeFlag desc, sort, id;

-- ------------------------------

-- 歌单名是否已经存在
-- @name count_by_name
-- @param user_id:i64
-- @param name:String
-- @param id:i64 排除的歌单ID(新增时为0)
-- @return i64
select count(*) from user_favorite where userId = :user_id and name = :name and isDelete = 0 and id != :id;

-- ------------------------------

-- 修改歌单名和简介
-- @name update_info
-- @param id:i64
-- @param name:String
-- @param description:String?
update user_favorite set name = :name, description = :description where id = :id;

-- ------------------------------

-- 修改封面
-- @name update_logo
-- @param id:i64
-- @param logo:String?
update user_favorite set logo = :logo where id = :id;

-- ------------------------------

-- 标记为删除
-- @name set_deleted
-- @param id:i64
-- @param delete_time:i64
update user_favorite set isDelete = 1, deleteTime = :delete_time where id = :id;

-- ------------------------------

-- 修改排序号
-- @name update_sort
-- @param id:i64
-- @param user_id:i64
-- @param sort:i64
update user_favorite set sort = :sort where id = :id and userId = :user_id;

-- ------------------------------

-- 歌单是否在首页推荐或轮播里(别人也可以查看)
-- @name count_public
-- @param id_text:String 歌单ID的文本
-- @return i64
select (select count(*) from useful where state = 1 and (',' || favoriteIds || ',') like '%,' || :id_text || ',%')
     + (select count(*) from swiper where state = 1 and swiperType = 1 and data = :id_text);

-- ------------------------------

-- 多个歌单(未删除)
-- @name select_by_ids
-- @param ids:List<i64>
-- @return List<UserFavorite>
select * from user_favorite where isDelete = 0
-- @each ids seq="," open="and id in (" close=")" item="id"
:id
-- @end
