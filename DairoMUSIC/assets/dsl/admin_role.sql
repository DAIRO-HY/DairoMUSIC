-- 所有角色
-- @name select_list
-- @return List<AdminRole>
select * from admin_role order by id;

-- ------------------------------

-- 角色名是否已经被其他角色使用
-- @name count_by_name
-- @param name:String
-- @param id:i64 排除的角色ID(新增时为0)
-- @return i64
select count(*) from admin_role where name = :name and id != :id;
