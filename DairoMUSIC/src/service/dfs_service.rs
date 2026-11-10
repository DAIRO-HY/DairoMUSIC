//! DairoDFS文件存储
//! - 上传:先按MD5秒传,存储里没有这个文件时再上传文件内容
//! - 访问:文件的公开链接是 {DFS对外地址}/d/{urlPath}{存储路径},不需要登录,支持断点续传

use crate::application;
use reqwest::StatusCode;
use std::sync::{LazyLock, RwLock};
use std::time::Duration;

/// HTTP客户端
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap()
});

/// 公开链接前缀 {DFS对外地址}/d/{urlPath},第一次用到时从DFS获取
static URL_PREFIX: RwLock<Option<String>> = RwLock::new(None);

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

/// 启动时获取公开链接前缀,DFS暂时连不上时之后用到的时候再获取
pub fn init() {
    tokio::spawn(async {
        if let Err(e) = refresh_url_prefix().await {
            eprintln!("-->获取DairoDFS公开链接前缀失败: {}", e);
        }
    });
}

/// 获取文件的公开访问地址
/// - path 存储路径,为空时返回None
pub fn file_url(path: Option<&str>) -> Option<String> {
    let path = path.filter(|it| !it.is_empty())?;
    if path.starts_with("http://") || path.starts_with("https://") {
        //采集平台的图片等外部地址
        return Some(path.to_string());
    }
    let prefix = URL_PREFIX.read().unwrap().clone();
    match prefix {
        Some(prefix) => Some(format!("{}{}", prefix, path)),
        None => {
            //还没有获取到前缀,后台再获取一次,这次先返回空
            init();
            None
        }
    }
}

/// 从DFS获取公开链接前缀
async fn refresh_url_prefix() -> Result<(), DfsError> {
    let res = request(CLIENT.get(api_url("/api/mine/info"))).await?;
    let info: serde_json::Value = res.json().await?;
    let url_path = info["urlPath"].as_str().unwrap_or_default();
    if url_path.is_empty() {
        return Err(DfsError("DFS账号还没有开启公开访问路径,请在DFS的“我的”页面里生成".to_string()));
    }
    let public_url = if application::ARGS.dfs_public_url.is_empty() {
        &application::ARGS.dfs_url
    } else {
        &application::ARGS.dfs_public_url
    };
    let prefix = format!("{}/d/{}", public_url.trim_end_matches('/'), url_path);
    *URL_PREFIX.write().unwrap() = Some(prefix);
    Ok(())
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
    format!("{}{}", application::ARGS.dfs_url.trim_end_matches('/'), path)
}

/// 发送请求,返回状态码和响应文本
async fn send(req: reqwest::RequestBuilder) -> Result<(StatusCode, String), DfsError> {
    let res = req.bearer_auth(&application::ARGS.dfs_token).send().await?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if status == StatusCode::UNAUTHORIZED {
        return Err(DfsError("API票据无效,请检查启动参数dfs-token".to_string()));
    }
    Ok((status, body))
}

/// 发送请求,不是200时返回错误
async fn request(req: reqwest::RequestBuilder) -> Result<reqwest::Response, DfsError> {
    let res = req.bearer_auth(&application::ARGS.dfs_token).send().await?;
    if res.status() == StatusCode::OK {
        return Ok(res);
    }
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if status == StatusCode::UNAUTHORIZED {
        return Err(DfsError("API票据无效,请检查启动参数dfs-token".to_string()));
    }
    Err(DfsError(format!("{}: {}", status, body)))
}
