-- 后台管理员登录记录
CREATE TABLE admin_token
(
    id      INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    token   VARCHAR(32) NOT NULL UNIQUE, -- 登录票据
    adminId INT8        NOT NULL, -- 管理员ID
    ip      VARCHAR(64) NOT NULL, -- 登录IP
    date    INT8        NOT NULL -- 登录时间(毫秒)
);
CREATE INDEX idx_admin_token_admin ON admin_token (adminId);
