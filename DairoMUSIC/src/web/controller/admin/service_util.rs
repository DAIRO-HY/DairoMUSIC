//! DairoDFS配置、采集服务配置页面共用的检查

use crate::web::biz_error::BizError;

/// 检查端口,为空时返回0(使用默认端口)
pub fn parse_port(field: &str, value: &str) -> Result<u16, BizError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(0);
    }
    match value.parse::<u16>() {
        Ok(it) if it >= 1 => Ok(it),
        _ => Err(BizError::field(field, "端口必须是1到65535之间的数字")),
    }
}

/// 检查地址,必须以http://或https://开头
pub fn check_url(field: &str, url: &str) -> Result<String, BizError> {
    let url = url.trim().trim_end_matches('/').to_string();
    if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(BizError::field(field, "地址必须以http://或https://开头"));
    }
    Ok(url)
}

/// 检查目录,必须是绝对路径
pub fn check_dir(field: &str, dir: &str) -> Result<String, BizError> {
    let dir = dir.trim().to_string();
    if !dir.is_empty() && !std::path::Path::new(&dir).is_absolute() {
        return Err(BizError::field(field, "请填写绝对路径"));
    }
    Ok(dir)
}

/// 检查客户端访问方式
pub fn check_public_mode(mode: &str) -> Result<String, BizError> {
    match mode {
        "" | "proxy" => Ok("proxy".to_string()),
        "direct" => Ok("direct".to_string()),
        _ => Err(BizError::field("publicMode", "访问方式不正确")),
    }
}
