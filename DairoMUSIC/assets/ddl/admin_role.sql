-- 后台管理员角色
CREATE TABLE admin_role
(
    id     INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    name   VARCHAR(20) NOT NULL UNIQUE, -- 角色名
    remark VARCHAR(100) -- 备注
);
