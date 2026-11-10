-- 获取角色可以使用的启用中的菜单
-- @name select_actives_by_role
-- @param role_id:i64
-- @return List<AdminMenuBar>
select m.* from admin_menu_bar m
inner join admin_permission p on p.menuId = m.id
where p.roleId = :role_id and m.state = 1
order by m.parentId, m.orderNum, m.id;

-- ------------------------------

-- 角色是否可以使用某个页面
-- @name count_role_url
-- @param role_id:i64
-- @param url:String
-- @return i64
select count(*) from admin_menu_bar m
inner join admin_permission p on p.menuId = m.id
where p.roleId = :role_id and m.state = 1 and m.url = :url;

-- ------------------------------

-- 所有菜单,带上级菜单名
-- @name select_list
-- @return List<SelectListTemp>
select m.id, m.name, m.ico, m.url, m.parentId, p.name as parentName, m.state, m.orderNum
from admin_menu_bar m left join admin_menu_bar p on m.parentId = p.id
order by case when m.parentId = 0 then m.orderNum else p.orderNum end, case when m.parentId = 0 then m.id else m.parentId end, m.parentId, m.orderNum, m.id;

-- ------------------------------

-- 所有顶级菜单(选择上级菜单用)
-- @name select_tops
-- @return List<AdminMenuBar>
select * from admin_menu_bar where parentId = 0 order by orderNum, id;

-- ------------------------------

-- 菜单名在同一级下是否已经存在
-- @name count_by_name
-- @param name:String
-- @param parent_id:i64
-- @param id:i64 排除的菜单ID(新增时为0)
-- @return i64
select count(*) from admin_menu_bar where name = :name and parentId = :parent_id and id != :id;

-- ------------------------------

-- 子菜单数量
-- @name count_children
-- @param id:i64
-- @return i64
select count(*) from admin_menu_bar where parentId = :id;
