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


-- ------------------------------

-- 歌单被首页的推荐分组、轮播使用的次数(包括禁用的分组和轮播)
-- @name count_referenced
-- @param id_text:String 歌单ID的文本
-- @return i64
select (select count(*) from useful where (',' || favoriteIds || ',') like '%,' || :id_text || ',%')
     + (select count(*) from swiper where swiperType = 1 and data = :id_text);

-- ------------------------------

-- 修改歌单所属的会员(0:归管理员)
-- @name set_owner
-- @param id:i64
-- @param user_id:i64
update user_favorite set userId = :user_id, sort = 0 where id = :id;

-- ------------------------------

-- 后台的歌单列表(分页,不包括已删除的歌单和"我喜欢")
-- @name page_admin
-- @page
-- @param id:i64?
-- @param name:String? 歌单名(模糊匹配)
-- @param user_id:i64? 所属会员ID,0:归管理员
-- @param home:i64? 1:只看首页使用的歌单
select f.id, f.name, f.logo, f.musicCount, f.userId, f.date,
       (select u.email from user u where u.id = f.userId) as userEmail,
       (select count(*) from useful where (',' || favoriteIds || ',') like '%,' || f.id || ',%')
           + (select count(*) from swiper where swiperType = 1 and data = cast(f.id as text)) as homeCount
from user_favorite f
where f.isDelete = 0 and f.likeFlag = 0
-- @if id != null
and f.id = :id
-- @end
-- @if name != null
and f.name like '%' || :name || '%'
-- @end
-- @if user_id != null
and f.userId = :user_id
-- @end
-- @if home != null
and ((select count(*) from useful where (',' || favoriteIds || ',') like '%,' || f.id || ',%')
    + (select count(*) from swiper where swiperType = 1 and data = cast(f.id as text))) > 0
-- @end

-- ------------------------------

-- 后台选择歌单时搜索(歌单名包含关键词,或者ID相同),最多20个
-- @name search_admin
-- @param key:String?
-- @param id:i64 关键词是数字时的歌单ID,不是数字时为0
-- @return List<SearchAdminTemp>
select f.id, f.name, f.logo, f.musicCount, f.userId,
       (select u.email from user u where u.id = f.userId) as userEmail
from user_favorite f
where f.isDelete = 0 and f.likeFlag = 0
-- @if key != null
and (f.name like '%' || :key || '%' or f.id = :id)
-- @end
order by f.id desc limit 20
