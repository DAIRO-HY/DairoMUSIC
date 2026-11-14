use crate::web::biz_error::BizError;
use crate::dao::PageIn;
use serde::{Deserialize, Serialize};

/// 业务错误时返回的数据结构
#[derive(Deserialize, Serialize, Debug)]
pub struct ResultData<T> {
    pub code: usize,
    pub msg: String,
    pub data: T,
}

/// 分页查询结果
#[derive(Serialize, Debug)]
pub struct PageData<T> {
    // 总条数
    pub total: i64,

    // 当前页的数据
    pub list: Vec<T>,
}

impl<T> PageData<T> {
    pub fn new(total: i64, list: Vec<T>) -> Self {
        Self { total, list }
    }
}

/// 分页查询参数(从URL参数获取)
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageForm {
    // 页码,从1开始
    pub page: Option<u32>,

    // 每页条数
    pub page_size: Option<u32>,

    // 排序字段
    pub sort_key: Option<String>,

    // 排序方式 asc/desc
    pub sort_type: Option<String>,
}

impl PageForm {
    /// 转换成DAO的分页参数
    /// - sort_keys 允许排序的字段,不在列表里的排序字段使用默认排序
    /// - default_sort 默认排序字段
    /// - default_type 默认排序方式
    pub fn to_page_in(&self, sort_keys: &[&str], default_sort: &str, default_type: &str) -> PageIn {
        let sort_key = self.sort_key.as_deref().filter(|it| sort_keys.contains(it));
        let (sort_key, sort_type) = match sort_key {
            Some(key) => {
                let sort_type = match self.sort_type.as_deref() {
                    Some("asc") => "asc",
                    _ => "desc",
                };
                (key, sort_type)
            }
            None => (default_sort, default_type),
        };
        PageIn {
            page: self.page.unwrap_or(1).max(1),
            page_size: self.page_size.unwrap_or(20).clamp(1, 200),
            sort_key: sort_key.to_string(),
            sort_type: sort_type.to_string(),
        }
    }
}

/// 下拉选项
#[derive(Serialize, Debug)]
pub struct OptionItem {
    pub value: i64,
    pub text: String,
}

/// 空字符串当作没有输入
pub fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|it| it.trim().to_string()).filter(|it| !it.is_empty())
}

/// 解析逗号分隔的ID(去掉重复的)
/// - name 提示里使用的名称,如"歌曲"
/// - max 最多允许的数量
pub fn parse_ids(ids: &str, name: &str, max: usize) -> Result<Vec<i64>, BizError> {
    let mut list = Vec::new();
    for it in ids.split(',').map(str::trim).filter(|it| !it.is_empty()) {
        let Ok(id) = it.parse::<i64>() else {
            return Err(BizError::msg(format!("{}ID格式错误", name)));
        };
        if !list.contains(&id) {
            list.push(id);
        }
    }

    //没有ID时批量更新的语句会更新所有数据,必须检查
    if list.is_empty() {
        return Err(BizError::msg(format!("请选择{}", name)));
    }
    if list.len() > max {
        return Err(BizError::msg(format!("一次最多选择{}条{}", max, name)));
    }
    Ok(list)
}
