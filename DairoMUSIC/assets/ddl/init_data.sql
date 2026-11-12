-- 全新安装时的初始数据

-- 超级管理员角色
INSERT INTO admin_role (id, name, remark) VALUES (1, '超级管理员', '拥有所有权限');

-- 菜单
INSERT INTO admin_menu_bar (id, name, ico, url, parentId, orderNum) VALUES
(1, '首页', 'house-door-fill', '/admin/index.html', 0, 10),
(2, '管理员', 'shield-lock-fill', NULL, 0, 20),
(3, '管理员列表', NULL, '/admin/admin_list.html', 2, 10),
(4, '角色列表', NULL, '/admin/admin_role_list.html', 2, 20),
(5, '会员管理', 'people-fill', NULL, 0, 30),
(6, '会员列表', NULL, '/admin/user_list.html', 5, 10),
(7, '歌曲管理', 'music-note-beamed', NULL, 0, 40),
(8, '歌曲列表', NULL, '/admin/music_list.html', 7, 10),
(9, '重复歌曲', NULL, '/admin/music_duplicate_list.html', 7, 20),
(10, '系统设置', 'gear-fill', NULL, 0, 999),
(11, '导航菜单', NULL, '/admin/admin_menu_list.html', 10, 10),
(12, '系统配置', NULL, '/admin/system_config.html', 10, 20),
(13, 'DairoDFS配置', NULL, '/admin/dfs_config.html', 10, 30),
(14, '采集服务配置', NULL, '/admin/search_config.html', 10, 40),
(15, '依赖软件', NULL, '/admin/software.html', 10, 50);

-- 超级管理员拥有所有菜单
INSERT INTO admin_permission (roleId, menuId) SELECT 1, id FROM admin_menu_bar;
