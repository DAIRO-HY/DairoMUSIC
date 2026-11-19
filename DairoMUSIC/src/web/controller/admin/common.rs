use crate::dao::{admin_dao, admin_menu_bar_dao, admin_token_dao, music_complaint_dao, music_correct_dao};
use crate::util::{pwd_util, request_util};
use crate::web::biz_error::BizError;
use crate::web::filter::auth::LoginAdmin;
use crate::web::form;
use crate::db;
use hyperoute_core::{Ext, Raw};
use hyperoute_macro::{GET, POST};
use serde::Serialize;
use validator::Validate;

/// 左侧菜单
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Menu {
    pub id: i64,

    // 菜单名称
    pub name: String,

    // 图标
    pub ico: Option<String>,

    // 页面链接
    pub url: Option<String>,

    // 子菜单
    pub children: Vec<Menu>,
}

/// 当前登录的管理员信息
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub name: String,
    pub real_name: Option<String>,
}

/// 获取当前角色可以使用的菜单(两级)
#[GET("/admin/api/common/menu")]
pub async fn menu(login: Ext<LoginAdmin>) -> Result<Vec<Menu>, BizError> {
    let mut ctx = lib_db::get_context();
    let list = admin_menu_bar_dao::select_actives_by_role(&mut ctx, login.role_id).await?;
    let (tops, subs): (Vec<_>, Vec<_>) = list.into_iter().partition(|it| it.parentId == 0);
    let menus = tops
        .into_iter()
        .map(|top| Menu {
            children: subs
                .iter()
                .filter(|it| it.parentId == top.id)
                .map(|it| Menu {
                    id: it.id,
                    name: it.name.clone(),
                    ico: it.ico.clone(),
                    url: it.url.clone(),
                    children: Vec::new(),
                })
                .collect(),
            id: top.id,
            name: top.name,
            ico: top.ico,
            url: top.url,
        })
        //没有链接也没有子菜单的分组不显示
        .filter(|it| it.url.is_some() || !it.children.is_empty())
        .collect();
    Ok(menus)
}

/// 待处理的事项数量,没有对应页面权限的不返回
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    // 还没有处理的纠错数量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correct: Option<i64>,

    // 还没有处理的投诉数量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complaint: Option<i64>,
}

/// 首页显示的待处理事项:还没有处理的纠错、投诉数量
#[GET("/admin/api/common/todo")]
pub async fn todo(login: Ext<LoginAdmin>) -> Result<Todo, BizError> {
    let mut ctx = lib_db::get_context();
    let mut todo = Todo { correct: None, complaint: None };
    if admin_menu_bar_dao::count_role_url(&mut ctx, login.role_id, "/admin/music_correct_list.html".to_string()).await? > 0 {
        todo.correct = Some(music_correct_dao::count_pending(&mut ctx).await?);
    }
    if admin_menu_bar_dao::count_role_url(&mut ctx, login.role_id, "/admin/music_complaint_list.html".to_string()).await? > 0 {
        todo.complaint = Some(music_complaint_dao::count_pending(&mut ctx).await?);
    }
    Ok(todo)
}

/// 运行环境的问题(首页醒目提示)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthIssue {
    // 有问题的软件或服务
    pub name: &'static str,

    // 问题说明
    pub msg: String,

    // 去设置的页面
    pub url: &'static str,
}

/// 检查运行需要的软件和服务:DairoDFS、ffmpeg、ffprobe,启用了歌曲采集时检查采集服务,内置采集服务还要检查yt-dlp、deno
/// 返回有问题的项,全部正常时返回空数组
#[GET("/admin/api/common/health")]
pub async fn health() -> Vec<HealthIssue> {
    use crate::service::software_service::{self, Software, DENO, DFS, FFMPEG, FFPROBE, SEARCH, YT_DLP};
    use crate::service::{collect_service, config_service, dfs_service};
    use std::time::Duration;

    const TIMEOUT: Duration = Duration::from_secs(5);
    const SOFTWARE_URL: &str = "/admin/software.html";
    let config = config_service::get();

    //工具不能运行
    let check_tool = |sw: &'static Software| async move {
        match software_service::version(sw).await {
            Ok(_) => None,
            Err(e) => Some(HealthIssue { name: sw.name, msg: format!("不能使用: {}", e), url: SOFTWARE_URL }),
        }
    };

    //DairoDFS
    let check_dfs = async {
        let url = "/admin/dfs_config.html";
        if config.dfs_builtin && !software_service::is_installed(&DFS) {
            return Some(HealthIssue { name: DFS.name, msg: "还没有安装内置的DairoDFS,歌曲、图片都不能保存".to_string(), url });
        }
        match tokio::time::timeout(TIMEOUT, dfs_service::test()).await {
            Ok(Ok(_)) => None,
            Ok(Err(e)) => Some(HealthIssue { name: DFS.name, msg: format!("连接失败,歌曲、图片都不能保存: {}", e), url }),
            Err(_) => Some(HealthIssue { name: DFS.name, msg: "连接超时,歌曲、图片都不能保存".to_string(), url }),
        }
    };

    //采集服务
    let check_collect = async {
        let url = "/admin/search_config.html";
        if !config.collect_enable {
            return None;
        }
        if config.collect_builtin && !software_service::is_installed(&SEARCH) {
            return Some(HealthIssue { name: SEARCH.name, msg: "已经启用了歌曲采集,但是还没有安装内置的采集服务".to_string(), url });
        }
        match tokio::time::timeout(TIMEOUT, collect_service::test()).await {
            Ok(Ok(_)) => None,
            Ok(Err(e)) => Some(HealthIssue { name: SEARCH.name, msg: format!("已经启用了歌曲采集,但是{}", e), url }),
            Err(_) => Some(HealthIssue { name: SEARCH.name, msg: "已经启用了歌曲采集,但是连接超时".to_string(), url }),
        }
    };
    let collect_tools = async {
        if config.collect_enable && config.collect_builtin {
            let (yt_dlp, deno) = tokio::join!(check_tool(&YT_DLP), check_tool(&DENO));
            vec![yt_dlp, deno]
        } else {
            Vec::new()
        }
    };
    let (dfs, ffmpeg, ffprobe, collect, tools) = tokio::join!(check_dfs, check_tool(&FFMPEG), check_tool(&FFPROBE), check_collect, collect_tools);
    [dfs, ffmpeg, ffprobe, collect].into_iter().chain(tools).flatten().collect()
}

/// 获取当前登录的管理员信息
#[GET("/admin/api/common/info")]
pub async fn info(login: Ext<LoginAdmin>) -> Result<Info, BizError> {
    let mut ctx = lib_db::get_context();
    let admin = admin_dao::select_one(&mut ctx, login.id).await?;
    Ok(Info {
        name: admin.name,
        real_name: admin.realName,
    })
}

/// 修改自己的密码,修改后其他设备上的登录全部失效,当前设备需要重新登录
#[POST("/admin/api/common/modify_pwd")]
pub async fn modify_pwd(login: Ext<LoginAdmin>, form: form::ModifyPwd) -> Result<Raw, BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let mut ctx = db::begin().await?;
    let admin = admin_dao::select_one(&mut ctx, login.id).await?;
    if !pwd_util::verify(&form.old_pwd, &admin.pwd).await {
        return Err(BizError::field("oldPwd", "当前密码不正确"));
    }
    let pwd = pwd_util::hash(&form.pwd).await;
    admin_dao::update_pwd(&mut ctx, login.id, pwd).await?;
    admin_token_dao::delete_by_admin_id(&mut ctx, login.id).await?;
    ctx.commit().await?;
    Ok(Raw::new("").header("set-cookie", request_util::clear_admin_token_cookie()))
}
