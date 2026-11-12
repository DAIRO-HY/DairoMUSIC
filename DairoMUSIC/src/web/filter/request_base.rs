use crate::web::biz_error::BizError;
use crate::web::request_base::{self, RequestBase};
use hyperoute_core::{Next, Request, Response};
use hyperoute_macro::FILTER;

/// 记录客户端访问DairoMUSIC使用的地址,接口返回DFS文件、采集服务的代理地址时使用
#[FILTER(order = 0, include = ["/api/**", "/admin/api/**"])]
pub async fn request_base(req: Request, next: Next) -> Result<Response, BizError> {
    let base = RequestBase::from_headers(req.headers());
    Ok(request_base::scope(base, next.run(req)).await)
}
