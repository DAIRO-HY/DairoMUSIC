-- 系统配置(键值对)
CREATE TABLE system_config
(
    name  VARCHAR(64) PRIMARY KEY NOT NULL, -- 配置名
    value TEXT        NOT NULL -- 配置值
);
