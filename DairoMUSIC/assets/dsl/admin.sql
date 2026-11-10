-- 通过登录名获取管理员
-- @name select_by_name
-- @param name:String
-- @return Admin?
select * from admin where name = :name;

-- ------------------------------

-- 管理员数量,用来判断系统是否已经初始化
-- @name count
-- @return i64
select count(*) from admin;

-- ------------------------------

-- 更新最后登录时间
-- @name update_last_login_time
-- @param id:i64
-- @param last_login_time:i64
update admin set lastLoginTime = :last_login_time where id = :id;

-- ------------------------------

-- 修改密码
-- @name update_pwd
-- @param id:i64
-- @param pwd:String
update admin set pwd = :pwd where id = :id;

-- ------------------------------

-- 管理员列表(分页)
-- @name page_list
-- @page
-- @param name:String? 登录名(模糊匹配)
-- @param role_id:i64?
select a.id, a.name, a.realName, a.roleId, r.name as roleName, a.state, a.date, a.lastLoginTime
from admin a left join admin_role r on a.roleId = r.id
where 1 = 1
-- @if name != null
and a.name like '%' || :name || '%'
-- @end
-- @if role_id != null
and a.roleId = :role_id
-- @end

-- ------------------------------

-- 登录名是否已经被其他管理员使用
-- @name count_by_name
-- @param name:String
-- @param id:i64 排除的管理员ID(新增时为0)
-- @return i64
select count(*) from admin where name = :name and id != :id;

-- ------------------------------

-- 使用某个角色的管理员数量
-- @name count_by_role
-- @param role_id:i64
-- @return i64
select count(*) from admin where roleId = :role_id;

-- ------------------------------

-- 更新管理员信息(不包括密码)
-- @name update_info
-- @param id:i64
-- @param name:String
-- @param real_name:String?
-- @param remark:String?
-- @param role_id:i64
-- @param state:i64
update admin set name = :name, realName = :real_name, remark = :remark, roleId = :role_id, state = :state where id = :id;
