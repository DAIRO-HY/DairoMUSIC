-- 后台左侧菜单
CREATE TABLE admin_menu_bar
(
    id       INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    name     VARCHAR(20) NOT NULL, -- 菜单名称
    ico      VARCHAR(200), -- 图标(bootstrap-icons的图标名,如house-fill)
    url      VARCHAR(200), -- 页面链接,有子菜单的分组为空
    parentId INT8        NOT NULL default 0, -- 上级菜单ID,0表示顶级
    state    INTEGER     NOT NULL default 1, -- 状态 0:禁用 1:启用
    orderNum INTEGER     NOT NULL default 0, -- 排序,越小越靠前
    UNIQUE (name, parentId)
);
