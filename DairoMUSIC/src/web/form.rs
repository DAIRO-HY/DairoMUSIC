use serde::Deserialize;
use validator::Validate;

// 表单结构体放在controller之外：hyperoute会为同文件内的请求体结构体生成multipart解析，
// 而multipart暂不支持Option字段，前端只提交urlencoded，所以这里只走serde解析

/// 后台登录
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AdminLogin {
    // 登录名
    #[validate(length(min = 1, message = "请输入登录名"))]
    pub name: String,

    // 密码
    #[validate(length(min = 1, message = "请输入密码"))]
    pub pwd: String,
}

/// 安装向导:创建超级管理员
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateAdmin {
    // 登录名
    #[validate(length(min = 2, max = 20, message = "登录名长度必须在2到20位之间"))]
    pub name: String,

    // 密码
    #[validate(length(min = 6, max = 32, message = "密码长度必须在6到32位之间"))]
    pub pwd: String,
}

/// 修改自己的密码
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ModifyPwd {
    // 当前密码
    #[validate(length(min = 1, message = "请输入当前密码"))]
    pub old_pwd: String,

    // 新密码
    #[validate(length(min = 6, max = 32, message = "密码长度必须在6到32位之间"))]
    pub pwd: String,
}

/// 管理员编辑
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AdminEdit {
    // ID,新增时为0
    #[serde(default)]
    pub id: i64,

    // 登录名
    #[validate(length(min = 2, max = 20, message = "登录名长度必须在2到20位之间"))]
    pub name: String,

    // 密码,修改时为空表示不修改
    #[serde(default)]
    #[validate(length(max = 32, message = "密码长度不能超过32位"))]
    pub pwd: String,

    // 姓名
    #[serde(default)]
    #[validate(length(max = 20, message = "姓名长度不能超过20位"))]
    pub real_name: String,

    // 备注
    #[serde(default)]
    #[validate(length(max = 200, message = "备注长度不能超过200位"))]
    pub remark: String,

    // 角色ID
    pub role_id: i64,

    // 状态 0:禁用 1:正常
    pub state: i64,
}

/// 角色编辑
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RoleEdit {
    // ID,新增时为0
    #[serde(default)]
    pub id: i64,

    // 角色名
    #[validate(length(min = 1, max = 20, message = "角色名长度必须在1到20位之间"))]
    pub name: String,

    // 备注
    #[serde(default)]
    #[validate(length(max = 100, message = "备注长度不能超过100位"))]
    pub remark: String,
}

/// 菜单编辑
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct MenuEdit {
    // ID,新增时为0
    #[serde(default)]
    pub id: i64,

    // 名称
    #[validate(length(min = 1, max = 20, message = "名称长度必须在1到20位之间"))]
    pub name: String,

    // 图标
    #[serde(default)]
    #[validate(length(max = 200, message = "图标长度不能超过200位"))]
    pub ico: String,

    // 链接
    #[serde(default)]
    #[validate(length(max = 200, message = "链接长度不能超过200位"))]
    pub url: String,

    // 上级菜单ID,0表示顶级
    #[serde(default)]
    pub parent_id: i64,

    // 状态 0:禁用 1:启用
    pub state: i64,

    // 排序号
    #[serde(default)]
    pub order_num: i64,
}

/// 会员编辑
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserEdit {
    // ID,新增时为0
    #[serde(default)]
    pub id: i64,

    // 邮箱
    #[validate(email(message = "邮箱格式不正确"), length(max = 128, message = "邮箱长度不能超过128位"))]
    pub email: String,

    // 密码,修改时为空表示不修改
    #[serde(default)]
    #[validate(length(max = 32, message = "密码长度不能超过32位"))]
    pub pwd: String,

    // 状态 0:禁用 1:正常 2:注销
    pub state: i64,

    // 是否信赖用户
    #[serde(default)]
    pub friend_flag: i64,
}

/// 歌曲编辑
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct MusicEdit {
    pub id: i64,

    // 歌名
    #[serde(default)]
    #[validate(length(max = 300, message = "歌名长度不能超过300位"))]
    pub name: String,

    // 歌手
    #[serde(default)]
    #[validate(length(max = 128, message = "歌手长度不能超过128位"))]
    pub artist: String,

    // 专辑
    #[serde(default)]
    #[validate(length(max = 128, message = "专辑长度不能超过128位"))]
    pub album: String,

    // 状态
    pub state: i64,
}

/// 重复歌曲去重
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicDistinct {
    // 要合并的歌曲ID,逗号分隔(包括保留的歌曲)
    pub ids: String,

    // 保留的歌曲ID
    pub best_id: i64,
}

/// 角色权限
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub role_id: i64,

    // 有权限的菜单ID,逗号分隔
    #[serde(default)]
    pub menu_ids: String,
}

/// 客户端登录时的设备信息
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    // 设备唯一标识
    #[validate(length(min = 1, max = 64, message = "设备标识不能为空,长度不能超过64位"))]
    pub device_id: String,

    // 客户端标识 0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux
    #[serde(default)]
    pub client_flag: i64,

    // 客户端版本号
    #[serde(default)]
    pub client_version: i64,
}

/// 客户端邮箱登录
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserLogin {
    #[validate(length(min = 1, message = "请输入邮箱"))]
    pub email: String,

    #[validate(length(min = 1, message = "请输入密码"))]
    pub pwd: String,

    // 设备唯一标识
    #[validate(length(min = 1, max = 64, message = "设备标识不能为空,长度不能超过64位"))]
    pub device_id: String,

    // 客户端标识 0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux
    #[serde(default)]
    pub client_flag: i64,

    // 客户端版本号
    #[serde(default)]
    pub client_version: i64,
}

/// 注册
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserRegister {
    #[validate(email(message = "邮箱格式不正确"), length(max = 128, message = "邮箱长度不能超过128位"))]
    pub email: String,

    #[validate(length(min = 6, max = 32, message = "密码长度必须在6到32位之间"))]
    pub pwd: String,

    // 邮箱验证码
    #[validate(length(min = 1, message = "请输入验证码"))]
    pub code: String,

    // 设备唯一标识
    #[validate(length(min = 1, max = 64, message = "设备标识不能为空,长度不能超过64位"))]
    pub device_id: String,

    // 客户端标识 0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux
    #[serde(default)]
    pub client_flag: i64,

    // 客户端版本号
    #[serde(default)]
    pub client_version: i64,
}

/// 重置密码
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ResetPwd {
    #[validate(email(message = "邮箱格式不正确"))]
    pub email: String,

    // 邮箱验证码
    #[validate(length(min = 1, message = "请输入验证码"))]
    pub code: String,

    // 新密码
    #[validate(length(min = 6, max = 32, message = "密码长度必须在6到32位之间"))]
    pub pwd: String,
}

/// 临时会员绑定邮箱
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct BindEmail {
    #[validate(email(message = "邮箱格式不正确"), length(max = 128, message = "邮箱长度不能超过128位"))]
    pub email: String,

    // 邮箱验证码
    #[validate(length(min = 1, message = "请输入验证码"))]
    pub code: String,

    // 登录密码
    #[validate(length(min = 6, max = 32, message = "密码长度必须在6到32位之间"))]
    pub pwd: String,
}

/// 发送验证码
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SendCode {
    #[validate(email(message = "邮箱格式不正确"))]
    pub email: String,

    // 验证码类型 register:注册 reset_pwd:重置密码 bind_email:绑定邮箱
    pub r#type: String,
}

/// 修改会员资料
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserEditInfo {
    // 昵称
    #[serde(default)]
    #[validate(length(max = 32, message = "昵称长度不能超过32位"))]
    pub nick: String,
}

/// 只需要密码的操作(注销账号)
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Pwd {
    pub pwd: String,
}

/// 新建歌单
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteAdd {
    #[validate(length(min = 1, max = 32, message = "歌单名长度必须在1到32位之间"))]
    pub name: String,
}

/// 修改歌单
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteEdit {
    pub id: i64,

    #[validate(length(min = 1, max = 32, message = "歌单名长度必须在1到32位之间"))]
    pub name: String,

    // 简介
    #[serde(default)]
    #[validate(length(max = 1000, message = "简介长度不能超过1000位"))]
    pub description: String,
}

/// 歌单排序
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteSort {
    // 排好序的歌单ID,逗号分隔
    pub ids: String,
}

/// 歌单里添加或删除歌曲
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteMusic {
    pub favorite_id: i64,
    pub music_id: i64,
}

/// 添加采集
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct MusicCollect {
    #[validate(length(min = 1, max = 300, message = "歌名长度必须在1到300位之间"))]
    pub name: String,

    #[serde(default)]
    pub logo: String,

    // 采集平台
    pub platform: String,

    // 采集参数
    #[validate(length(min = 1, max = 128, message = "采集参数不正确"))]
    pub extra: String,
}

/// 歌曲纠错
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct MusicCorrect {
    pub id: i64,

    #[serde(default)]
    #[validate(length(max = 300, message = "歌名长度不能超过300位"))]
    pub name: String,

    #[serde(default)]
    #[validate(length(max = 128, message = "歌手长度不能超过128位"))]
    pub artist: String,

    #[serde(default)]
    #[validate(length(max = 128, message = "专辑长度不能超过128位"))]
    pub album: String,
}

/// 内容反馈(歌曲投诉、意见反馈)
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    // 歌曲ID(歌曲投诉时)
    #[serde(default)]
    pub id: i64,

    #[validate(length(min = 1, max = 1000, message = "内容长度必须在1到1000位之间"))]
    pub content: String,
}

/// 系统配置
#[derive(Deserialize, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SystemConfig {
    // 是否开放注册 1:开放 0:关闭
    #[serde(default)]
    pub allow_register: i64,

    #[serde(default)]
    #[validate(length(max = 128, message = "长度不能超过128位"))]
    pub smtp_host: String,

    #[validate(range(min = 1, message = "端口不正确"))]
    pub smtp_port: u16,

    #[serde(default)]
    #[validate(length(max = 128, message = "长度不能超过128位"))]
    pub smtp_user: String,

    // 为空表示不修改
    #[serde(default)]
    pub smtp_pwd: String,

    #[serde(default)]
    #[validate(length(max = 64, message = "长度不能超过64位"))]
    pub smtp_from_name: String,

    // 是否采集视频 1:采集 0:不采集
    #[serde(default)]
    pub video_enable: i64,

    #[serde(default)]
    pub video_encoder: String,

    #[serde(default)]
    #[validate(range(min = 0, max = 64, message = "线程数必须在0到64之间"))]
    pub video_threads: i64,

    // 是否允许保存到相册 1:允许 0:不允许
    #[serde(default)]
    pub allow_save_album: i64,

    // 是否允许下载歌曲 1:允许 0:不允许
    #[serde(default)]
    pub allow_download_music: i64,

    // 是否允许下载视频 1:允许 0:不允许
    #[serde(default)]
    pub allow_download_video: i64,
}

/// DairoDFS配置
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DfsConfig {
    // 1:使用内置的DFS 0:使用外部的DFS
    #[serde(default)]
    pub builtin: i64,

    // 外部DFS的接口地址
    #[serde(default)]
    #[validate(length(max = 256, message = "长度不能超过256位"))]
    pub url: String,

    // 外部DFS的API票据,为空时不修改
    #[serde(default)]
    #[validate(length(max = 128, message = "长度不能超过128位"))]
    pub token: String,

    // 客户端访问方式 proxy:通过DairoMUSIC代理 direct:直接访问
    #[serde(default)]
    pub public_mode: String,

    // 对外的访问地址(direct时使用)
    #[serde(default)]
    #[validate(length(max = 256, message = "长度不能超过256位"))]
    pub public_url: String,

    // 内置DFS的端口,为空时使用默认端口
    #[serde(default)]
    pub port: String,

    // 内置DFS的数据目录
    #[serde(default)]
    #[validate(length(max = 512, message = "长度不能超过512位"))]
    pub data_dir: String,

    // 内置DFS的文件存储目录
    #[serde(default)]
    #[validate(length(max = 512, message = "长度不能超过512位"))]
    pub storage_dir: String,

    // 内置DFS的管理员用户名
    #[serde(default)]
    #[validate(length(max = 32, message = "长度不能超过32位"))]
    pub admin_name: String,

    // 内置DFS的管理员密码,为空时不修改
    #[serde(default)]
    #[validate(length(max = 64, message = "长度不能超过64位"))]
    pub admin_pwd: String,
}

/// 采集服务配置
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SearchConfig {
    // 是否启用歌曲采集 1:启用 0:不启用
    #[serde(default)]
    pub enable: i64,

    // 1:使用内置的采集服务 0:使用外部的采集服务
    #[serde(default)]
    pub builtin: i64,

    // 外部采集服务的地址
    #[serde(default)]
    #[validate(length(max = 256, message = "长度不能超过256位"))]
    pub url: String,

    // 客户端访问方式 proxy:通过DairoMUSIC代理 direct:直接访问
    #[serde(default)]
    pub public_mode: String,

    // 对外的访问地址(direct时使用)
    #[serde(default)]
    #[validate(length(max = 256, message = "长度不能超过256位"))]
    pub public_url: String,

    // 内置采集服务的端口,为空时使用默认端口
    #[serde(default)]
    pub port: String,

    // 内置采集服务的数据目录
    #[serde(default)]
    #[validate(length(max = 512, message = "长度不能超过512位"))]
    pub data_dir: String,

    // 同时下载歌曲的数量,为空时为1
    #[serde(default)]
    pub thread_count: String,
}
