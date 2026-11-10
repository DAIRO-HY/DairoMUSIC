-- 通过登录票据获取登录记录
-- @name select_by_token
-- @param token:String
-- @return UserLogin?
select * from user_login where token = :token;

-- ------------------------------

-- 删除设备上的登录记录(同一台设备只保留一条)
-- @name delete_by_device
-- @param user_id:i64
-- @param device_id:String
delete from user_login where userId = :user_id and deviceId = :device_id;

-- ------------------------------

-- 删除登录记录(退出登录)
-- @name delete_by_token
-- @param token:String
delete from user_login where token = :token;

-- ------------------------------

-- 删除会员的所有登录记录(修改密码、注销时)
-- @name delete_by_user
-- @param user_id:i64
delete from user_login where userId = :user_id;

-- ------------------------------

-- 删除会员的其他设备上的登录记录(修改密码时保留当前设备)
-- @name delete_others
-- @param user_id:i64
-- @param token:String
delete from user_login where userId = :user_id and token != :token;
