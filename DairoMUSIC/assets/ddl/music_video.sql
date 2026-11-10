-- 歌曲视频(原画、流畅各一条;原画不高于流畅的规格时只有原画)
CREATE TABLE music_video
(
    id         INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    musicId    INT8         NOT NULL, -- 所属歌曲ID
    quality    INTEGER      NOT NULL, -- 清晰度 1:原画 2:流畅
    state      INTEGER      NOT NULL default 0, -- 状态 0:转码中 1:可以播放 2:转码失败
    width      INTEGER      NOT NULL default 0, -- 宽
    height     INTEGER      NOT NULL default 0, -- 高
    codec      VARCHAR(16), -- 视频编码 h264、hevc
    bitRate    INTEGER, -- 比特率(kbps)
    frameRate  INTEGER, -- 帧率
    duration   INT8, -- 时长(毫秒)
    size       INT8, -- 文件大小
    md5        VARCHAR(32), -- 文件MD5
    path       VARCHAR(512), -- 存储位置
    memo       VARCHAR(512), -- 处理说明(失败原因等)
    createTime INT8         NOT NULL, -- 创建时间(毫秒)
    UNIQUE (musicId, quality)
);
