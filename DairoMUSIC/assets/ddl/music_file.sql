-- 歌曲文件(同一首歌不同音质各一条)
CREATE TABLE music_file
(
    id                INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    musicId           INT8         NOT NULL, -- 所属歌曲ID
    bitRate           INTEGER, -- 比特率 128:普通 192:中等 320:高音质 999:无损
    sampleRate        INTEGER, -- 采样率
    dataStartPosition INTEGER, -- 第一帧数据开始位置
    sampleCount       INTEGER, -- 采样数
    frameLength       INTEGER, -- 帧长,包括帧头
    size              INT8         NOT NULL, -- 文件大小
    format            VARCHAR(10)  NOT NULL, -- 文件格式 mp3、wav、ape、flac、mp4等
    md5               VARCHAR(32) UNIQUE, -- 除去文本信息部分的文件MD5
    path              VARCHAR(512) NOT NULL, -- 存储位置
    UNIQUE (musicId, bitRate)
);
