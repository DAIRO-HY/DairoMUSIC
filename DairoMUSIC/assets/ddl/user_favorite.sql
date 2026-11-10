-- 会员创建的收藏(歌单)
CREATE TABLE user_favorite
(
    id          INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    name        VARCHAR(64) NOT NULL, -- 收藏名
    description VARCHAR(1024), -- 描述
    logo        VARCHAR(512), -- 封面
    isDelete    INTEGER     NOT NULL default 0, -- 是否删除
    deleteTime  INT8, -- 删除时间(毫秒)
    likeFlag    INTEGER     NOT NULL default 0, -- 注册时默认创建的"我喜欢",不允许删除和改名
    date        INT8        NOT NULL, -- 创建时间(毫秒)
    sort        INTEGER     NOT NULL default 0, -- 排序,越小越靠前
    musicCount  INTEGER     NOT NULL default 0, -- 歌曲数
    userId      INT8        NOT NULL -- 所属会员ID
);
CREATE INDEX idx_user_favorite_user ON user_favorite (userId);
