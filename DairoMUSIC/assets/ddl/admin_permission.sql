-- 角色可以使用的菜单
CREATE TABLE admin_permission
(
    roleId INT8 NOT NULL, -- 角色ID
    menuId INT8 NOT NULL, -- 菜单ID
    PRIMARY KEY (roleId, menuId)
);
CREATE INDEX idx_admin_permission_menu ON admin_permission (menuId);
