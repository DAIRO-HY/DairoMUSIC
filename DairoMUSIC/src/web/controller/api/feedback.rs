//! 客户端:意见反馈

use crate::dao::feedback_dao;
use crate::dao::feedback_dao::Feedback;
use crate::util::time_util;
use crate::web::biz_error::BizError;
use crate::web::filter::user_auth::LoginUser;
use crate::web::form;
use hyperoute_core::Ext;
use hyperoute_macro::POST;
use validator::Validate;

/// 提交意见反馈
/// - client_flag 客户端标识 0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux
#[POST("/api/feedback/add")]
pub async fn add(login: Ext<LoginUser>, form: form::Content, client_flag: Option<i64>) -> Result<String, BizError> {
    if let Err(e) = form.validate() {
        return Err(BizError::fields(e));
    }
    let feedback = Feedback {
        userId: login.id,
        content: form.content,
        clientFlag: client_flag,
        date: time_util::current_millis(),
        ..Default::default()
    };
    let mut ctx = lib_db::get_context();
    feedback_dao::insert(&mut ctx, &feedback).await?;
    Ok("您的宝贵意见已经收到,我们会及时处理,感谢您使用我们的产品。".to_string())
}
