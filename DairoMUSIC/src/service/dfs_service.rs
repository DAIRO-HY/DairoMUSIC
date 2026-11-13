//! DairoDFS文件存储
//! - 上传:先按MD5秒传,存储里没有这个文件时再上传文件内容
//! - 访问:文件的公开链接是 {DFS对外地址}/d/{urlPath}{存储路径},不需要登录,支持断点续传
//! - 对外地址:默认通过DairoMUSIC代理(/proxy/dairo-dfs),客户端只需要访问DairoMUSIC的域名;也可以设置成直接访问DFS
//! - 内置DFS:第一次启动时自动初始化(设置存储目录、创建管理员),并自动生成API票据

use crate::service::config_service::{self, Config};
use crate::service::process_service;
use crate::service::software_service::DFS;
use crate::web::request_base;
use rand::distr::{Alphanumeric, SampleString};
use reqwest::StatusCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, RwLock};
use std::time::Duration;

/// 代理DFS文件的路径前缀
pub const PROXY_PREFIX: &str = "/proxy/dairo-dfs";

/// HTTP客户端
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap()
});

/// DFS账号的公开访问路径(urlPath),第一次用到时从DFS获取
static URL_PATH: RwLock<Option<String>> = RwLock::new(None);

/// 是否正在获取公开访问路径
static REFRESHING: AtomicBool = AtomicBool::new(false);

/// DFS接口调用失败
#[derive(Debug)]
pub struct DfsError(pub String);

impl std::fmt::Display for DfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DairoDFS: {}", self.0)
    }
}

impl From<reqwest::Error> for DfsError {
    fn from(e: reqwest::Error) -> Self {
        DfsError(e.to_string())
    }
}

/// 接口地址(服务端调用),内置DFS使用本机地址
pub fn api_base(config: &Config) -> String {
    if config.dfs_builtin {
        format!("http://127.0.0.1:{}", config_service::dfs_port(config))
    } else {
        config.dfs_url.trim().trim_end_matches('/').to_string()
    }
}

/// API票据
fn token(config: &Config) -> String {
    if config.dfs_builtin {
        config.dfs_builtin_token.clone()
    } else {
        config.dfs_token.clone()
    }
}

/// 启动时获取公开访问路径,DFS暂时连不上时之后用到的时候再获取
pub fn init() {
    if REFRESHING.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async {
        if let Err(e) = refresh_url_path().await {
            eprintln!("-->获取DairoDFS公开访问路径失败: {}", e);
        }
        REFRESHING.store(false, Ordering::SeqCst);
    });
}

/// 配置修改之后,重新获取公开访问路径
pub fn reset() {
    *URL_PATH.write().unwrap() = None;
    init();
}

/// 客户端访问DFS的地址
fn public_base(config: &Config) -> String {
    if config.dfs_public_mode != "direct" {
        //通过DairoMUSIC代理,和访问DairoMUSIC使用同一个域名
        let base = request_base::current().map(|it| it.origin()).unwrap_or_default();
        return format!("{}{}", base, PROXY_PREFIX);
    }
    let url = config.dfs_public_url.trim().trim_end_matches('/');
    if !url.is_empty() {
        return url.to_string();
    }
    if config.dfs_builtin {
        //内置DFS:客户端访问DairoMUSIC的主机名 + DFS的端口
        let port = config_service::dfs_port(config);
        return match request_base::current() {
            Some(it) => format!("{}://{}:{}", it.scheme, it.host_name(), port),
            None => format!("http://127.0.0.1:{}", port),
        };
    }
    api_base(config)
}

/// 获取文件的公开访问地址
/// - path 存储路径,为空时返回None
pub fn file_url(path: Option<&str>) -> Option<String> {
    let path = path.filter(|it| !it.is_empty())?;
    if path.starts_with("http://") || path.starts_with("https://") {
        //采集平台的图片等外部地址
        return Some(path.to_string());
    }
    let url_path = URL_PATH.read().unwrap().clone();
    match url_path {
        Some(url_path) => Some(format!("{}/d/{}{}", public_base(&config_service::get()), url_path, path)),
        None => {
            //还没有获取到公开访问路径,后台再获取一次,这次先返回空
            init();
            None
        }
    }
}

/// 从DFS获取公开访问路径
async fn refresh_url_path() -> Result<(), DfsError> {
    if token(&config_service::get()).is_empty() {
        return Err(DfsError("还没有配置DFS,请在后台「DairoDFS配置」里设置".to_string()));
    }
    let res = request(CLIENT.get(api_url("/api/mine/info"))).await?;
    let info: serde_json::Value = res.json().await?;
    let url_path = info["urlPath"].as_str().unwrap_or_default();
    if url_path.is_empty() {
        return Err(DfsError("DFS账号还没有开启公开访问路径,请在DFS的“我的”页面里生成".to_string()));
    }
    *URL_PATH.write().unwrap() = Some(url_path.to_string());
    Ok(())
}

/// 测试连接:返回DFS的用户名和公开访问路径
pub async fn test() -> Result<String, DfsError> {
    let config = config_service::get();
    if token(&config).is_empty() {
        return Err(DfsError(if config.dfs_builtin { "内置DFS还没有初始化完成".to_string() } else { "还没有设置API票据".to_string() }));
    }
    let res = request(CLIENT.get(api_url("/api/mine/info")).timeout(Duration::from_secs(10))).await?;
    let info: serde_json::Value = res.json().await?;
    let url_path = info["urlPath"].as_str().unwrap_or_default();
    if url_path.is_empty() {
        return Err(DfsError("连接成功,但DFS账号还没有开启公开访问路径,请在DFS的“我的”页面里生成".to_string()));
    }
    *URL_PATH.write().unwrap() = Some(url_path.to_string());
    Ok(format!("连接成功,DFS用户: {}", info["name"].as_str().unwrap_or_default()))
}

/// 上传文件
/// - data 文件内容
/// - path 存储路径,如 /image/logo/2024/xxx.png
/// - content_type 文件类型,为空时DFS按扩展名判断
pub async fn upload(data: Vec<u8>, path: &str, content_type: &str) -> Result<(), DfsError> {
    let md5 = format!("{:x}", md5::compute(&data));

    //先按MD5秒传
    let res = CLIENT
        .post(api_url("/api/file_upload/by_md5"))
        .query(&[("md5", md5.as_str()), ("path", path), ("contentType", content_type)]);
    match send(res).await? {
        (StatusCode::OK, _) => return Ok(()),
        (StatusCode::NOT_FOUND, _) => {}
        (status, body) => return Err(DfsError(format!("秒传失败({}): {}", status, body))),
    }

    //存储里没有这个文件,上传文件内容
    let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
    let folder = if folder.is_empty() { "/" } else { folder };
    let part = reqwest::multipart::Part::bytes(data).file_name(name.to_string());
    let form = reqwest::multipart::Form::new().part("file", part);
    let res = CLIENT
        .post(api_url("/api/file_upload"))
        .query(&[("folder", folder), ("contentType", content_type)])
        .multipart(form)
        .timeout(Duration::from_secs(600));
    match send(res).await? {
        (StatusCode::OK, _) => Ok(()),
        (status, body) => Err(DfsError(format!("上传失败({}): {}", status, body))),
    }
}

/// 上传本地文件(边读边上传,大文件也不会占用很多内存)
/// - file 本地文件
/// - md5 文件的MD5
/// - path 存储路径
pub async fn upload_file(file: &std::path::Path, md5: &str, path: &str, content_type: &str) -> Result<(), DfsError> {
    //先按MD5秒传
    let res = CLIENT
        .post(api_url("/api/file_upload/by_md5"))
        .query(&[("md5", md5), ("path", path), ("contentType", content_type)]);
    match send(res).await? {
        (StatusCode::OK, _) => return Ok(()),
        (StatusCode::NOT_FOUND, _) => {}
        (status, body) => return Err(DfsError(format!("秒传失败({}): {}", status, body))),
    }
    let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
    let folder = if folder.is_empty() { "/" } else { folder };
    let reader = tokio::fs::File::open(file).await.map_err(|e| DfsError(e.to_string()))?;
    let size = reader.metadata().await.map_err(|e| DfsError(e.to_string()))?.len();
    let stream = tokio_util::io::ReaderStream::new(reader);
    let part = reqwest::multipart::Part::stream_with_length(reqwest::Body::wrap_stream(stream), size).file_name(name.to_string());
    let form = reqwest::multipart::Form::new().part("file", part);
    let res = CLIENT
        .post(api_url("/api/file_upload"))
        .query(&[("folder", folder), ("contentType", content_type)])
        .multipart(form)
        .timeout(Duration::from_secs(3600));
    match send(res).await? {
        (StatusCode::OK, _) => Ok(()),
        (status, body) => Err(DfsError(format!("上传失败({}): {}", status, body))),
    }
}

/// 拼接DFS接口地址
fn api_url(path: &str) -> String {
    format!("{}{}", api_base(&config_service::get()), path)
}

/// API票据无效时的提示
fn unauthorized_msg() -> String {
    if config_service::get().dfs_builtin {
        "API票据无效,请在后台「DairoDFS配置」里重新初始化".to_string()
    } else {
        "API票据无效,请在后台「DairoDFS配置」里检查API票据".to_string()
    }
}

/// 发送请求,返回状态码和响应文本
async fn send(req: reqwest::RequestBuilder) -> Result<(StatusCode, String), DfsError> {
    let res = req.bearer_auth(token(&config_service::get())).send().await?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if status == StatusCode::UNAUTHORIZED {
        return Err(DfsError(unauthorized_msg()));
    }
    Ok((status, body))
}

/// 发送请求,不是200时返回错误
async fn request(req: reqwest::RequestBuilder) -> Result<reqwest::Response, DfsError> {
    let res = req.bearer_auth(token(&config_service::get())).send().await?;
    if res.status() == StatusCode::OK {
        return Ok(res);
    }
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if status == StatusCode::UNAUTHORIZED {
        return Err(DfsError(unauthorized_msg()));
    }
    Err(DfsError(format!("{}: {}", status, body)))
}

/// 内置DFS启动后调用:第一次启动时自动初始化,没有API票据时自动生成
pub async fn init_builtin() {
    match do_init_builtin().await {
        Ok(()) => {
            process_service::set_message(&DFS, "");
            reset();
        }
        Err(e) => {
            let msg = format!("初始化失败: {}", e);
            process_service::log(&DFS, &msg);
            process_service::set_message(&DFS, msg);
        }
    }
}

async fn do_init_builtin() -> Result<(), String> {
    let config = config_service::get();
    let base = api_base(&config);

    //等待DFS启动完成(能响应请求),启动后尽快创建管理员
    let mut initialized = None;
    for _ in 0..300 {
        if !process_service::is_running(&DFS) {
            return Ok(());
        }
        if let Ok(res) = CLIENT.get(format!("{}/api/install/info", base)).timeout(Duration::from_secs(3)).send().await {
            //还没有初始化时返回200,已经初始化之后返回错误
            initialized = Some(res.status() != StatusCode::OK);
            break;
        }
        if process_service::info(&DFS).status != "running" {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let Some(initialized) = initialized else {
        return Err("等待DFS启动超时".to_string());
    };
    let name = config_service::dfs_admin_name(&config);
    if !initialized {
        process_service::log(&DFS, "第一次启动,自动初始化");
        let storage = config_service::dfs_storage_dir(&config);
        std::fs::create_dir_all(&storage).map_err(|e| format!("创建存储目录{}失败: {}", storage.display(), e))?;
        let folders = storage.to_string_lossy().to_string();
        post_form(&base, "/api/install/set_storage", None, &[("folders", &folders)]).await?;

        //先保存密码再创建管理员,避免创建之后保存失败,密码丢失
        let pwd = if config.dfs_admin_pwd.is_empty() {
            let pwd = Alphanumeric.sample_string(&mut rand::rng(), 16);
            config_service::update(|it| it.dfs_admin_pwd = pwd.clone()).await.map_err(|e| e.to_string())?;
            pwd
        } else {
            config.dfs_admin_pwd.clone()
        };
        //管理员只能创建一次,失败说明已经被别人创建了
        post_form(&base, "/api/install/create_admin", None, &[("name", &name), ("pwd", &pwd)])
            .await
            .map_err(|e| format!("创建管理员失败,DFS的管理员可能已经被别人创建,请检查DFS: {}", e))?;
        process_service::log(&DFS, format!("已创建管理员: {},存储目录: {}", name, folders));
    }

    //已经有API票据并且可以使用时不需要再生成
    let config = config_service::get();
    if !config.dfs_builtin_token.is_empty() {
        let res = CLIENT
            .get(format!("{}/api/mine/info", base))
            .bearer_auth(&config.dfs_builtin_token)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if res.status() == StatusCode::OK {
            return Ok(());
        }
    }
    if config.dfs_admin_pwd.is_empty() {
        return Err("DFS已经初始化过,请在下方填写DFS管理员的用户名和密码".to_string());
    }

    //登录,生成API票据和公开访问路径
    let res = post_form(
        &base,
        "/api/login/do_login",
        None,
        &[("name", &name), ("pwd", &config.dfs_admin_pwd), ("deviceId", "DairoMUSIC")],
    )
    .await
    .map_err(|e| format!("用管理员{}登录DFS失败,请检查用户名和密码: {}", name, e))?;
    let login: serde_json::Value = serde_json::from_str(&res).map_err(|e| e.to_string())?;
    let login_token = login["token"].as_str().unwrap_or_default().to_string();
    let mut info = mine_info(&base, &login_token).await?;
    if info["apiToken"].as_str().unwrap_or_default().is_empty() {
        put(&base, "/api/mine/api_token?flag=1", &login_token).await?;
        info = mine_info(&base, &login_token).await?;
    }
    if info["urlPath"].as_str().unwrap_or_default().is_empty() {
        put(&base, "/api/mine/url_path?flag=1", &login_token).await?;
    }

    //DairoMUSIC自己处理歌曲和视频,DFS不需要再处理上传的文件(生成缩略图、转码等)
    let id = info["id"].as_i64().unwrap_or_default().to_string();
    let email = info["email"].as_str().unwrap_or_default().to_string();
    let edit = post_form(
        &base,
        "/api/user/edit",
        Some(&login_token),
        &[("id", &id), ("name", &name), ("email", &email), ("state", "1"), ("handleFile", "0")],
    )
    .await;
    if let Err(e) = edit {
        process_service::log(&DFS, format!("设置不处理上传的文件失败: {}", e));
    }
    let api_token = info["apiToken"].as_str().unwrap_or_default().to_string();
    if api_token.is_empty() {
        return Err("生成API票据失败".to_string());
    }
    config_service::update(|it| it.dfs_builtin_token = api_token).await.map_err(|e| e.to_string())?;
    process_service::log(&DFS, "已生成API票据");
    Ok(())
}

/// 提交表单,返回响应文本
async fn post_form(base: &str, path: &str, token: Option<&str>, form: &[(&str, &str)]) -> Result<String, String> {
    let mut req = CLIENT.post(format!("{}{}", base, path)).form(form).timeout(Duration::from_secs(30));
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if status != StatusCode::OK {
        return Err(format!("{} {}: {}", path, status, body));
    }
    Ok(body)
}

/// PUT请求
async fn put(base: &str, path: &str, token: &str) -> Result<(), String> {
    let res = CLIENT
        .put(format!("{}{}", base, path))
        .bearer_auth(token)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status() != StatusCode::OK {
        return Err(format!("{} {}: {}", path, res.status(), res.text().await.unwrap_or_default()));
    }
    Ok(())
}

/// 获取当前登录用户的信息
async fn mine_info(base: &str, token: &str) -> Result<serde_json::Value, String> {
    let res = CLIENT
        .get(format!("{}/api/mine/info", base))
        .bearer_auth(token)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status() != StatusCode::OK {
        return Err(format!("/api/mine/info {}: {}", res.status(), res.text().await.unwrap_or_default()));
    }
    res.json().await.map_err(|e| e.to_string())
}
