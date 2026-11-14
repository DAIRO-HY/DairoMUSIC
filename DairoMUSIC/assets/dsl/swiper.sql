-- 启用中的轮播
-- @name select_active
-- @return List<Swiper>
select * from swiper where state = 1 order by sort, id;


-- ------------------------------

-- 后台的轮播列表
-- @name select_list
-- @return List<Swiper>
select * from swiper order by sort, id;
