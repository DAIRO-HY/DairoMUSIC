-- 通过登录票据获取登录记录
-- @name select_by_token
-- @param token:String
-- @return AdminToken?
select * from admin_token where token = :token;

-- ------------------------------

-- 删除登录记录(退出登录)
-- @name delete_by_token
-- @param token:String
delete from admin_token where token = :token;

-- ------------------------------

-- 删除管理员的所有登录记录(禁用、修改密码时)
-- @name delete_by_admin_id
-- @param admin_id:i64
delete from admin_token where adminId = :admin_id;

-- ------------------------------

-- 删除过期的登录记录
-- @name delete_expired
-- @param expire_time:i64
delete from admin_token where date < :expire_time;
