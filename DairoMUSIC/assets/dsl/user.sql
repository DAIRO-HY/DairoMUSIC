-- 会员列表(分页)
-- @name page_list
-- @page
-- @param id:i64?
-- @param email:String? 邮箱(模糊匹配)
-- @param state:i64?
-- @return User
select * from user where 1 = 1
-- @if id != null
and id = :id
-- @end
-- @if email != null
and email like '%' || :email || '%'
-- @end
-- @if state != null
and state = :state
-- @end

-- ------------------------------

-- 邮箱是否已经被其他会员使用
-- @name count_by_email
-- @param email:String
-- @param id:i64 排除的会员ID(新增时为0)
-- @return i64
select count(*) from user where email = :email and id != :id;

-- ------------------------------

-- 后台更新会员信息(不包括密码)
-- @name update_by_admin
-- @param id:i64
-- @param email:String
-- @param state:i64
-- @param friend_flag:i64
update user set email = :email, state = :state, friendFlag = :friend_flag where id = :id;

-- ------------------------------

-- 修改VIP
-- @name update_vip
-- @param id:i64
-- @param vip_time:i64 到期时间(毫秒),0:不是VIP
-- @param vip_forever:i64 是否永久VIP
update user set vipTime = :vip_time, vipForever = :vip_forever where id = :id;

-- ------------------------------

-- 修改密码
-- @name update_pwd
-- @param id:i64
-- @param pwd:String
update user set pwd = :pwd where id = :id;

-- ------------------------------

-- 通过邮箱获取会员
-- @name select_by_email
-- @param email:String
-- @return User?
select * from user where email = :email;

-- ------------------------------

-- 更新最后在线时间
-- @name update_last_login_time
-- @param id:i64
-- @param last_login_time:i64
update user set lastLoginTime = :last_login_time where id = :id;

-- ------------------------------

-- 修改昵称
-- @name update_nick
-- @param id:i64
-- @param nick:String?
update user set nick = :nick where id = :id;

-- ------------------------------

-- 修改头像
-- @name update_logo
-- @param id:i64
-- @param logo:String?
update user set logo = :logo where id = :id;

-- ------------------------------

-- 临时会员绑定邮箱,成为正式会员
-- @name bind_email
-- @param id:i64
-- @param email:String
-- @param pwd:String
update user set email = :email, pwd = :pwd, tempFlag = 0 where id = :id;

-- ------------------------------

-- 注销:状态改为2,邮箱和手机号加上后缀,之后可以用同一个邮箱重新注册
-- @name unregister
-- @param id:i64
-- @param suffix:String
update user set state = 2,
                email = case when email is null then null else email || :suffix end,
                phone = case when phone is null then null else phone || :suffix end
where id = :id;
