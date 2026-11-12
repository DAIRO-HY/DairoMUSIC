use clap::Parser;
use std::sync::LazyLock;
use tokio::sync::Notify;

/// 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 用来接收关闭通知的全局异步通知器
pub static SHUTDOWN_NOTIFY: Notify = Notify::const_new();

/// 程序启动参数
pub static ARGS: LazyLock<Argument> = LazyLock::new(|| Argument::parse());

/// 开发时使用工作空间根目录下的data目录(由build.rs注入)
#[cfg(debug_assertions)]
const DEFAULT_DATA_DIR: &str = env!("DAIRO_MUSIC_DEV_DATA_DIR");

/// 发布时使用程序启动目录下的data目录
#[cfg(not(debug_assertions))]
const DEFAULT_DATA_DIR: &str = "./data";

#[derive(Parser, Debug)]
#[command(name = "DairoMUSIC", version, about = "DairoMUSIC")]
pub struct Argument {
    /// WEB端口
    #[arg(long, env = "DAIRO_MUSIC_WEB_PORT", default_value = "8020")]
    pub web_port: u16,

    /// 数据保存目录(数据库文件等都在这个目录下)
    #[arg(long, env = "DAIRO_MUSIC_DATA_DIR", default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,

    /// 模拟发送邮件(只用于开发和测试):验证码不发送邮件,只记录到日志,
    /// 并且可以从本机通过 GET /api/dev/email_code?email= 获取。正式环境不要使用
    #[arg(long, env = "DAIRO_MUSIC_MOCK_EMAIL")]
    pub mock_email: bool,
}
