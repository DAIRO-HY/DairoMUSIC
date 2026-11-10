-- 歌手
CREATE TABLE artist
(
    id              INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    name            VARCHAR(32) NOT NULL, -- 歌手名、乐队名
    name2           VARCHAR(32), -- 歌手第二名称(英文名等)
    pinyin          VARCHAR(64), -- 拼音
    bandMemberCount INTEGER     NOT NULL default 1, -- 乐队人数
    lang            VARCHAR(32), -- 语言(华语、粤语)
    style           VARCHAR(128), -- 风格(摇滚、爵士等)
    sex             INTEGER     NOT NULL default -1, -- 0:女 1:男 -1:未知
    logo            VARCHAR(512), -- 头像
    description     VARCHAR(3000) -- 歌手介绍
);
