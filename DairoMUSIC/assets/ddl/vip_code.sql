-- VIP兑换码
CREATE TABLE vip_code
(
    id         INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    cardNo     VARCHAR(16) NOT NULL UNIQUE, -- 卡号(6位日期+10位随机数)
    cardPwd    VARCHAR(32) NOT NULL, -- 卡密
    batchNo    VARCHAR(14) NOT NULL, -- 批次号(生成时间,如20261114123400)
    days       INTEGER     NOT NULL, -- 兑换之后VIP的天数,0:永久
    expireTime INT8        NOT NULL, -- 兑换码的有效期(毫秒),过期之后不能兑换
    state      INTEGER     NOT NULL default 0, -- 状态 0:未使用 1:已使用 2:已作废
    userId     INT8, -- 兑换的会员ID
    useTime    INT8, -- 兑换时间(毫秒)
    date       INT8        NOT NULL -- 生成时间(毫秒)
);
CREATE INDEX idx_vip_code_batch ON vip_code (batchNo);
CREATE INDEX idx_vip_code_state ON vip_code (state);
CREATE INDEX idx_vip_code_user ON vip_code (userId);
