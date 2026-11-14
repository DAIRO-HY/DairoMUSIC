-- 会员
CREATE TABLE user
(
    id            INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    email         VARCHAR(128) UNIQUE, -- 邮箱
    nick          VARCHAR(32), -- 昵称
    sex           INTEGER NOT NULL default 0, -- 性别 0:保密 1:女 2:男
    phone         VARCHAR(20) UNIQUE, -- 手机号
    pwd           VARCHAR(128), -- 密码(argon2哈希)
    logo          VARCHAR(512), -- 头像
    state         INTEGER NOT NULL default 1, -- 状态 0:禁用 1:正常
    lastLoginTime INT8    NOT NULL default 0, -- 最后登录时间(毫秒)
    date          INT8    NOT NULL, -- 注册时间(毫秒)
    tempFlag      INTEGER NOT NULL default 0, -- 是否临时用户
    friendFlag    INTEGER NOT NULL default 0, -- 是否信赖用户,拥有一些特殊权限
    testFlag      INTEGER NOT NULL default 0, -- 应用市场审核专用的测试用户
    vipTime       INT8    NOT NULL default 0, -- VIP到期时间(毫秒),0:不是VIP
    vipForever    INTEGER NOT NULL default 0 -- 是否永久VIP
);
