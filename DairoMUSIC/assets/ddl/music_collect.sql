-- 歌曲采集记录
CREATE TABLE music_collect
(
    id            INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    musicId       INT8    NOT NULL UNIQUE, -- 歌曲ID
    downloadCount INTEGER NOT NULL default 0, -- 下载次数,次数过多时放弃下载
    url           VARCHAR(2048), -- 采集URL
    extra         VARCHAR(128) UNIQUE, -- 采集参数
    platform      VARCHAR(32), -- 采集平台
    date          INT8    NOT NULL, -- 采集时间(毫秒)
    state         INTEGER NOT NULL default -1, -- -1:未采集 0:采集中 1:采集成功 2:采集失败
    memo          TEXT, -- 采集进度说明,包括错误记录
    updateTime    INT8    NOT NULL -- 最后更新时间(毫秒)
);
CREATE INDEX idx_music_collect_download ON music_collect (downloadCount);
CREATE INDEX idx_music_collect_state ON music_collect (state);
CREATE INDEX idx_music_collect_update ON music_collect (updateTime);
