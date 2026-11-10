-- 歌曲纠错
CREATE TABLE music_correct
(
    id      INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    musicId INT8          NOT NULL, -- 歌曲ID
    data    VARCHAR(3000) NOT NULL, -- 要更新的数据,JSON格式
    state   INTEGER       NOT NULL default 0, -- 处理结果 0:未处理 1:处理完成
    userId  INT8          NOT NULL, -- 提交纠错的会员ID
    adminId INT8, -- 处理的管理员ID
    date    INT8          NOT NULL -- 创建时间(毫秒)
);
CREATE INDEX idx_music_correct_state ON music_correct (state);
