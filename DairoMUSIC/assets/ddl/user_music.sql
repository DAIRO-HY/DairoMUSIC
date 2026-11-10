-- 收藏里的歌曲
CREATE TABLE user_music
(
    favoriteId INT8    NOT NULL, -- 所属收藏ID
    musicId    INT8    NOT NULL, -- 歌曲ID
    userId     INT8    NOT NULL, -- 所属会员ID
    nick       VARCHAR(128), -- 自定义的歌曲名称
    sort       INTEGER NOT NULL default 0, -- 排序,越小越靠前
    PRIMARY KEY (favoriteId, musicId)
);
CREATE INDEX idx_user_music_user ON user_music (userId);
