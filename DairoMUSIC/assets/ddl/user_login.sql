-- 会员登录记录(每台设备一条)
CREATE TABLE user_login
(
    id            INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    token         VARCHAR(32) NOT NULL UNIQUE, -- 登录票据
    userId        INT8        NOT NULL, -- 会员ID
    deviceId      VARCHAR(64) NOT NULL, -- 设备唯一标识
    clientFlag    INTEGER     NOT NULL, -- 客户端标识 0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux
    clientVersion INTEGER     NOT NULL default 0, -- 客户端版本号
    ip            VARCHAR(64) NOT NULL, -- 登录IP
    date          INT8        NOT NULL, -- 登录时间(毫秒)
    UNIQUE (userId, deviceId)
);
