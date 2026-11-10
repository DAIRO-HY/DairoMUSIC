-- 角色有权限的菜单ID
-- @name select_menu_ids
-- @param role_id:i64
-- @return List<i64>
select menuId from admin_permission where roleId = :role_id;

-- ------------------------------

-- 删除角色的所有权限
-- @name delete_by_role
-- @param role_id:i64
delete from admin_permission where roleId = :role_id;

-- ------------------------------

-- 删除菜单的所有权限
-- @name delete_by_menu
-- @param menu_id:i64
delete from admin_permission where menuId = :menu_id;
