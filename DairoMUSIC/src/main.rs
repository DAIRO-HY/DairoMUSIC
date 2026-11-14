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

    //启动内置的DairoDFS和采集服务,获取DairoDFS的公开访问路径
    service::process_service::init();
    service::dfs_service::init();

    //后台处理采集任务和会员上传的歌曲
    service::collect_task::start();
    service::video_task::start();
    service::music_service::start_purge_task();

    //定时清理过期的登录记录
    web::filter::auth::start_clean_task();

    //收到Ctrl+C或SIGTERM(如systemctl stop)时退出
    tokio::spawn(async {
        wait_exit_signal().await;
        application::SHUTDOWN_NOTIFY.notify_waiters();
    });

    //等待程序退出
    application::SHUTDOWN_NOTIFY.notified().await;
    println!("-->即将退出");

    //停止内置的DairoDFS和采集服务
    service::process_service::stop_all().await;

    // 等待一段时间再退出确保监听端口都已经关闭
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    println!("-->程序已退出");
    Ok(())
}

/// 等待退出信号
async fn wait_exit_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let Ok(mut term) = signal(SignalKind::terminate()) else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}
