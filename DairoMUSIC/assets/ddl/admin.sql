-- 后台管理员
CREATE TABLE admin
(
    id            INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    name          VARCHAR(20)  NOT NULL UNIQUE, -- 登录名
    pwd           VARCHAR(128) NOT NULL, -- 登录密码(argon2哈希)
    roleId        INT8         NOT NULL, -- 角色ID
    realName      VARCHAR(20), -- 真实姓名
    remark        VARCHAR(200), -- 备注
    state         INTEGER      NOT NULL default 1, -- 状态 0:禁用 1:正常
    date          INT8         NOT NULL, -- 开通日期(毫秒)
    lastLoginTime INT8         NOT NULL default 0 -- 最后登录时间(毫秒)
);
CREATE INDEX idx_admin_role ON admin (roleId);
