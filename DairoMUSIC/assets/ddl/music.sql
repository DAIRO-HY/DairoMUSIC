-- 歌曲
CREATE TABLE music
(
    id         INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    name       VARCHAR(300), -- 歌名
    artist     VARCHAR(128), -- 歌手
    album      VARCHAR(128), -- 专辑
    logo       VARCHAR(512), -- 封面图片
    duration   INTEGER, -- 时长(毫秒)
    tag        VARCHAR(64), -- 标签
    year       INTEGER, -- 年代
    playCount  INTEGER NOT NULL default 0, -- 播放次数
    sourceType INTEGER NOT NULL, -- 歌曲来源 0:后台上传 1:会员上传 2:采集平台
    date       INT8    NOT NULL, -- 采集/添加时间(毫秒)
    collectState INTEGER NOT NULL default 0, -- 采集状态 0:采集中 1:采集失败 100:采集完成
    auditState INTEGER NOT NULL default 0, -- 审核状态 0:待审核 1:通过 2:拒绝(APP搜索不显示)
    maxBitRate INTEGER NOT NULL default 0, -- 最大比特率
    testFlag   INTEGER NOT NULL default 0, -- 应用市场审核专用的测试歌曲
    deleteFlag INTEGER NOT NULL default 0, -- 是否删除
    deleteTime INT8, -- 删除时间(毫秒)
    videoState INTEGER NOT NULL default 0, -- 视频状态 0:未采集 1:采集中 2:有视频 3:没有视频(只有一张图片) 4:后台删除了视频 5:采集失败
    videoTime  INT8 -- 视频状态的修改时间(毫秒),采集失败后按这个时间判断是否重试
);
CREATE INDEX idx_music_name ON music (name);
CREATE INDEX idx_music_artist ON music (artist);
CREATE INDEX idx_music_tag ON music (tag);
CREATE INDEX idx_music_collected ON music (collectState);
CREATE INDEX idx_music_audit_state ON music (auditState);
CREATE INDEX idx_music_delete ON music (deleteFlag);
CREATE INDEX idx_music_video_state ON music (videoState);
