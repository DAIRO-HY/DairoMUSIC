-- 歌曲投诉
CREATE TABLE music_complaint
(
    id      INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    userId  INT8          NOT NULL, -- 用户ID
    musicId INT8          NOT NULL, -- 歌曲ID
    content VARCHAR(1000) NOT NULL, -- 投诉内容
    state   INTEGER       NOT NULL default 0, -- 处理状态 0:未处理 1:已处理
    date    INT8          NOT NULL -- 创建时间(毫秒)
);
