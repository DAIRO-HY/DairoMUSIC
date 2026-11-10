mod application;
mod dao;
mod db;
mod service;
mod util;
mod web;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("-->DairoMUSIC {} 启动中", application::VERSION);

    //数据保存目录不存在时创建
    if let Err(e) = std::fs::create_dir_all(&application::ARGS.data_dir) {
        eprintln!("-->创建数据保存目录:{}失败: {}", application::ARGS.data_dir, e);
        std::process::exit(1);
    }
    lib_db::init(&application::ARGS.data_dir).await;

    //加载系统配置
    service::config_service::init().await;
    web::server::ready();

    //获取DairoDFS的公开链接前缀
    service::dfs_service::init();

    //后台处理采集任务和会员上传的歌曲
    service::collect_task::start();
    service::video_task::start();

    //定时清理过期的登录记录
    web::filter::auth::start_clean_task();

    //收到Ctrl+C时退出
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            application::SHUTDOWN_NOTIFY.notify_waiters();
        }
    });

    //等待程序退出
    application::SHUTDOWN_NOTIFY.notified().await;
    println!("-->即将退出");

    // 等待一段时间再退出确保监听端口都已经关闭
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    println!("-->程序已退出");
    Ok(())
}
