-- 首页轮播
CREATE TABLE swiper
(
    id    INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    name  VARCHAR(128)  NOT NULL, -- 名称
    image VARCHAR(1024) NOT NULL, -- 图片地址
    data  VARCHAR(1024), -- 数据
    swiperType INTEGER  NOT NULL, -- 类型 1:推荐的收藏 2:推荐单曲 3:拨打电话 4:跳转网页
    state INTEGER       NOT NULL default 1, -- 状态 0:禁用 1:启用
    sort  INTEGER       NOT NULL default 0, -- 排序,越小越靠前
    date  INT8          NOT NULL -- 创建时间(毫秒)
);
