-- 最近一次发送的验证码
-- @name select_last_code
-- @param email:String
-- @param email_type:String
-- @return Email?
select * from email where email = :email and emailType = :email_type and code is not null order by id desc limit 1;

-- ------------------------------

-- 最近一段时间内给这个邮箱发送的验证码数量
-- @name count_since
-- @param email:String
-- @param since:i64
-- @return i64
select count(*) from email where email = :email and date > :since;

-- ------------------------------

-- 最近一段时间内一共发送的邮件数量(防止被用来群发邮件)
-- @name count_all_since
-- @param since:i64
-- @return i64
select count(*) from email where date > :since;

-- ------------------------------

-- 更新发送结果
-- @name update_state
-- @param id:i64
-- @param state:i64
-- @param error:String?
-- @param send_time:i64
update email set state = :state, error = :error, sendTime = :send_time where id = :id;

-- ------------------------------

-- 验证码使用后作废
-- @name clear_code
-- @param id:i64
update email set code = null where id = :id;

-- ------------------------------

-- 最近一次发送的还有效的验证码(不分类型,开发测试用)
-- @name select_last_valid_code
-- @param email:String
-- @param since:i64
-- @return Email?
select * from email where email = :email and code is not null and date > :since order by id desc limit 1;
