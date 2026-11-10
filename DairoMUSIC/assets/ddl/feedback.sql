-- 用户反馈
CREATE TABLE feedback
(
    id           INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    userId       INT8          NOT NULL, -- 用户ID
    content      VARCHAR(1000) NOT NULL, -- 反馈内容
    reply        VARCHAR(1000), -- 回复内容
    replyAdminId INT8, -- 回复者ID
    replyTime    INT8, -- 回复时间(毫秒)
    clientFlag   INTEGER, -- 客户端标识 0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux
    date         INT8          NOT NULL -- 创建时间(毫秒)
);
