-- 首页推荐的歌单分组
CREATE TABLE useful
(
    id          INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    name        VARCHAR(16), -- 分组名
    showType    INTEGER       NOT NULL, -- 显示类型 1:表格 2:横向滚动 3:压缩分组
    favoriteIds VARCHAR(1024) NOT NULL, -- 收藏ID列表,逗号分隔
    sort        INTEGER       NOT NULL default 0, -- 排序,越小越靠前
    state       INTEGER       NOT NULL default 1 -- 状态 0:禁用 1:启用
);
