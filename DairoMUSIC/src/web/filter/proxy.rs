use crate::service::{collect_service, config_service, dfs_service};
use crate::web::biz_error::BizError;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::{Bytes, Frame};
use hyper::{Method, StatusCode};
use hyperoute_core::{Next, Request, Response};
use hyperoute_macro::FILTER;
use std::sync::LazyLock;
use std::time::Duration;
use tokio_stream::StreamExt;

/// 转发给DFS和采集服务的请求头
const REQUEST_HEADERS: [&str; 6] = ["range", "if-range", "if-none-match", "if-modified-since", "accept", "user-agent"];

/// 返回给客户端的响应头
const RESPONSE_HEADERS: [&str; 10] = [
    "content-type",
    "content-length",
    "content-range",
    "accept-ranges",
    "etag",
    "last-modified",
    "cache-control",
    "expires",
    "content-disposition",
    "location",
];

/// 转发请求的HTTP客户端(文件可能很大,不设置总的超时时间)
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
});

/// 代理DFS和采集服务的文件:客户端通过DairoMUSIC的域名访问,不需要再单独开放DFS和采集服务的端口
/// - /proxy/dairo-dfs/d/... 转发到 {DFS}/d/...
/// - /proxy/dairo-search/d/... 转发到 {采集服务}/d/...
/// 只转发公开文件的路径(/d/),DFS和采集服务的其他接口(如安装、设置cookie)不能通过代理访问
#[FILTER(order = 0, include = ["/proxy/**"])]
pub async fn proxy(req: Request, _next: Next) -> Result<Response, BizError> {
    //使用原始的路径和参数,不需要重新编码
    let path_and_query = req.uri().path_and_query().map(|it| it.as_str()).unwrap_or_default();
    let config = config_service::get();
    let (base, rest) = if let Some(rest) = path_and_query.strip_prefix(dfs_service::PROXY_PREFIX) {
        (dfs_service::api_base(&config), rest)
    } else if let Some(rest) = path_and_query.strip_prefix(collect_service::PROXY_PREFIX) {
        (collect_service::api_url(), rest)
    } else {
        return Err(BizError::not_found());
    };
    if !rest.starts_with("/d/") || base.is_empty() {
        return Err(BizError::not_found());
    }
    let method = req.method().clone();
    if method != Method::GET && method != Method::HEAD {
        return Err(BizError::not_found());
    }
    let mut forward = CLIENT.request(method, format!("{}{}", base, rest));
    for name in REQUEST_HEADERS {
        if let Some(value) = req.headers().get(name) {
            forward = forward.header(name, value.as_bytes());
        }
    }
    let res = match forward.send().await {
        Ok(it) => it,
        Err(e) => {
            eprintln!("-->代理请求失败: {}{} {}", base, rest, e);
            return Ok(status_response(StatusCode::BAD_GATEWAY));
        }
    };
    let mut builder = hyper::Response::builder().status(res.status().as_u16());
    for name in RESPONSE_HEADERS {
        if let Some(value) = res.headers().get(name) {
            builder = builder.header(name, value.as_bytes());
        }
    }

    //边读边返回,读取放到单独的任务里,客户端断开时停止读取
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<Bytes>>(8);
    tokio::spawn(async move {
        let mut stream = res.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(std::io::Error::other);
            let is_err = chunk.is_err();
            if tx.send(chunk).await.is_err() || is_err {
                break;
            }
        }
    });
    let body = StreamBody::new(tokio_stream::wrappers::ReceiverStream::new(rx).map(|it| it.map(Frame::data)));
    Ok(builder.body(BodyExt::boxed(body)).unwrap_or_else(|_| status_response(StatusCode::BAD_GATEWAY)))
}

/// 只有状态码的响应
fn status_response(status: StatusCode) -> Response {
    let mut res = hyper::Response::new(hyperoute_core::boxed_full(http_body_util::Full::new(Bytes::new())));
    *res.status_mut() = status;
    res
}
