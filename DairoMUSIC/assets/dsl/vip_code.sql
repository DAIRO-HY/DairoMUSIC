-- 后台的兑换码列表(分页)
-- @name page_list
-- @page
-- @param batch_no:String? 批次号
-- @param card_no:String? 卡号(模糊匹配)
-- @param state:i64?
-- @param user_id:i64? 兑换的会员ID
-- @param expire_before:i64? 只看这个时间之前过期的
-- @param expire_after:i64? 只看这个时间之后还没有过期的
select c.id, c.cardNo, c.cardPwd, c.batchNo, c.days, c.expireTime, c.state, c.userId, c.useTime, c.date,
       (select u.email from user u where u.id = c.userId) as userEmail
from vip_code c
where 1 = 1
-- @if batch_no != null
and c.batchNo = :batch_no
-- @end
-- @if card_no != null
and c.cardNo like '%' || :card_no || '%'
-- @end
-- @if state != null
and c.state = :state
-- @end
-- @if user_id != null
and c.userId = :user_id
-- @end
-- @if expire_before != null
and c.expireTime < :expire_before
-- @end
-- @if expire_after != null
and c.expireTime >= :expire_after
-- @end

-- ------------------------------

-- 通过卡号获取兑换码
-- @name select_by_card_no
-- @param card_no:String
-- @return VipCode?
select * from vip_code where cardNo = :card_no;

-- ------------------------------

-- 某个批次的兑换码数量
-- @name count_by_batch
-- @param batch_no:String
-- @return i64
select count(*) from vip_code where batchNo = :batch_no;

-- ------------------------------

-- 某个批次的所有兑换码(导出)
-- @name select_by_batch
-- @param batch_no:String
-- @return List<VipCode>
select * from vip_code where batchNo = :batch_no order by id;

-- ------------------------------

-- 兑换:标记为已使用(只有未使用的才能兑换)
-- @name set_used
-- @param id:i64
-- @param user_id:i64
-- @param use_time:i64
update vip_code set state = 1, userId = :user_id, useTime = :use_time where id = :id and state = 0;

-- ------------------------------

-- 作废没有使用的兑换码
-- @name set_void
-- @param ids:List<i64>
update vip_code set state = 2 where state = 0
-- @each ids seq="," open="and id in (" close=")" item="id"
:id
-- @end

-- ------------------------------

-- 作废整个批次里没有使用的兑换码
-- @name set_void_batch
-- @param batch_no:String
update vip_code set state = 2 where state = 0 and batchNo = :batch_no;
