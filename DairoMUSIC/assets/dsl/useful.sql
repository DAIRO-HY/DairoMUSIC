-- 启用中的推荐分组
-- @name select_active
-- @return List<Useful>
select * from useful where state = 1 order by sort, id;
