-- 歌词
CREATE TABLE music_lrc
(
    id            INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    musicId       INT8        NOT NULL default 0, -- 歌曲ID
    title         VARCHAR(32) NOT NULL, -- 歌词标题
    lrc           TEXT        NOT NULL, -- 歌词内容
    agreeCount    INTEGER     NOT NULL default 0, -- 赞同数
    disagreeCount INTEGER     NOT NULL default 0, -- 不赞同数
    state         INTEGER     NOT NULL default 0, -- 歌词状态 0:未审核 1:审核通过
    source        VARCHAR(32) -- 歌词来源
);
CREATE INDEX idx_music_lrc_music ON music_lrc (musicId);
