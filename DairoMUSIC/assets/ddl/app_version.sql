-- 客户端版本
CREATE TABLE app_version
(
    id          INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    clientFlag  INTEGER      NOT NULL, -- 客户端标识 0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux
    versionName VARCHAR(16)  NOT NULL, -- 版本名称
    versionCode INTEGER      NOT NULL, -- 版本号
    releaseType INTEGER      NOT NULL, -- 发行类型 1:稳定版 2:测试版 3:抢先版
    installType INTEGER      NOT NULL default 1, -- 安装方式 1:手动安装 2:弹框提示,后台下载 3:强制更新,后台下载
    instruction VARCHAR(1000), -- 更新说明
    url         VARCHAR(500) NOT NULL, -- 下载链接
    date        INT8         NOT NULL, -- 发布时间(毫秒)
    state       INTEGER      NOT NULL default 1, -- 1:有效 0:无效
    UNIQUE (clientFlag, versionCode)
);
