use rust_embed::RustEmbed;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Connection, SqliteConnection, SqlitePool};
use sqlx_context::DbContext;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(RustEmbed)]
#[folder = "../DairoMUSIC/assets/ddl/"]
struct Assets;

/// 数据库版本号
const VERSION: i32 = 9;

/// 数据库文件名
pub const DB_FILE_NAME: &str = "dairo-music.sqlite";

/// 建表顺序(init_data.sql是全新安装时的初始数据,放在最后)
const SQL_FILES: [&str; 24] = [
    "admin.sql",
    "admin_token.sql",
    "admin_role.sql",
    "admin_permission.sql",
    "admin_menu_bar.sql",
    "app_version.sql",
    "artist.sql",
    "email.sql",
    "feedback.sql",
    "music.sql",
    "music_collect.sql",
    "music_complaint.sql",
    "music_correct.sql",
    "music_file.sql",
    "music_lrc.sql",
    "music_video.sql",
    "swiper.sql",
    "system_config.sql",
    "useful.sql",
    "user.sql",
    "user_favorite.sql",
    "user_login.sql",
    "user_music.sql",
    "vip_code.sql",
];

/// 版本1升级到版本2:客户端编号统一为 0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux
/// 老的编号是 1:Windows 2:Mac 3:Linux 4:Android 5:Ios
const UPGRADE_V2: &str = "
alter table app_version rename column client to clientFlag;
update app_version set clientFlag = case clientFlag when 1 then 3 when 2 then 4 when 3 then 5 when 4 then 1 when 5 then 2 else clientFlag end;
alter table feedback rename column client to clientFlag;
update feedback set clientFlag = case clientFlag when 1 then 3 when 2 then 4 when 3 then 5 when 4 then 1 when 5 then 2 else clientFlag end;
";

/// 版本2升级到版本3:增加歌曲视频
const UPGRADE_V3: &str = "
alter table music add column videoState INTEGER NOT NULL default 0;
alter table music add column videoTime INT8;
CREATE INDEX idx_music_video_state ON music (videoState);
";

/// 版本3升级到版本4:系统设置下增加DairoDFS配置、采集服务配置、依赖软件三个页面,超级管理员拥有这些菜单
const UPGRADE_V4: &str = "
INSERT INTO admin_menu_bar (name, ico, url, parentId, orderNum)
SELECT it.name, NULL, it.url, (SELECT id FROM admin_menu_bar WHERE name = '系统设置' AND parentId = 0), it.orderNum
FROM (SELECT 'DairoDFS配置' AS name, '/admin/dfs_config.html' AS url, 30 AS orderNum
      UNION ALL SELECT '采集服务配置', '/admin/search_config.html', 40
      UNION ALL SELECT '依赖软件', '/admin/software.html', 50) it
WHERE EXISTS (SELECT 1 FROM admin_menu_bar WHERE name = '系统设置' AND parentId = 0)
  AND NOT EXISTS (SELECT 1 FROM admin_menu_bar m WHERE m.url = it.url);
INSERT OR IGNORE INTO admin_permission (roleId, menuId)
SELECT 1, id FROM admin_menu_bar WHERE url IN ('/admin/dfs_config.html', '/admin/search_config.html', '/admin/software.html');
";

/// 版本4升级到版本5:审核状态从state字段分出来,新增auditState(0:待审核 1:通过 2:拒绝),state只记录采集状态(0:采集中 1:采集失败 100:采集完成)
/// 原来的state 2:审核中 3:审核失败 100:正常
const UPGRADE_V5: &str = "
alter table music add column auditState INTEGER NOT NULL default 0;
update music set auditState = case state when 100 then 1 when 3 then 2 else 0 end;
update music set state = 100 where state in (2, 3);
CREATE INDEX idx_music_audit_state ON music (auditState);
";

/// 版本5升级到版本6:歌曲的state只表示采集状态,改名为collectState
const UPGRADE_V6: &str = "
alter table music rename column state to collectState;
DROP INDEX idx_music_state;
CREATE INDEX idx_music_collected ON music (collectState);
";

/// 版本6升级到版本7:增加「发现页管理」菜单(轮播图、推荐分组、歌单列表),超级管理员拥有这些菜单
const UPGRADE_V7: &str = "
INSERT INTO admin_menu_bar (name, ico, url, parentId, orderNum)
SELECT '发现页管理', 'compass-fill', NULL, 0, 50
WHERE NOT EXISTS (SELECT 1 FROM admin_menu_bar WHERE name = '发现页管理' AND parentId = 0);
INSERT INTO admin_menu_bar (name, ico, url, parentId, orderNum)
SELECT it.name, NULL, it.url, (SELECT id FROM admin_menu_bar WHERE name = '发现页管理' AND parentId = 0), it.orderNum
FROM (SELECT '轮播图' AS name, '/admin/swiper_list.html' AS url, 10 AS orderNum
      UNION ALL SELECT '推荐分组', '/admin/useful_list.html', 20
      UNION ALL SELECT '歌单列表', '/admin/favorite_list.html', 30) it
WHERE NOT EXISTS (SELECT 1 FROM admin_menu_bar m WHERE m.url = it.url);
INSERT OR IGNORE INTO admin_permission (roleId, menuId)
SELECT 1, id FROM admin_menu_bar
WHERE (name = '发现页管理' AND parentId = 0) OR url IN ('/admin/swiper_list.html', '/admin/useful_list.html', '/admin/favorite_list.html');
";

/// 版本7升级到版本8:会员增加VIP到期时间、是否永久VIP;新增「VIP兑换码」菜单(vip_code表在升级时用建表文件创建)
const UPGRADE_V8: &str = "
ALTER TABLE user ADD COLUMN vipTime INT8 NOT NULL default 0;
ALTER TABLE user ADD COLUMN vipForever INTEGER NOT NULL default 0;
INSERT INTO admin_menu_bar (name, ico, url, parentId, orderNum)
SELECT 'VIP兑换码', NULL, '/admin/vip_code_list.html', (SELECT parentId FROM admin_menu_bar WHERE url = '/admin/user_list.html'), 20
WHERE EXISTS (SELECT 1 FROM admin_menu_bar WHERE url = '/admin/user_list.html')
  AND NOT EXISTS (SELECT 1 FROM admin_menu_bar WHERE url = '/admin/vip_code_list.html');
INSERT OR IGNORE INTO admin_permission (roleId, menuId)
SELECT 1, id FROM admin_menu_bar WHERE url = '/admin/vip_code_list.html';
";

/// 版本8升级到版本9:纠错、投诉增加处理时间,投诉增加处理的管理员;新增「歌曲纠错」「歌曲投诉」菜单
const UPGRADE_V9: &str = "
ALTER TABLE music_correct ADD COLUMN handleTime INT8;
ALTER TABLE music_complaint ADD COLUMN adminId INT8;
ALTER TABLE music_complaint ADD COLUMN handleTime INT8;
CREATE INDEX idx_music_correct_music ON music_correct (musicId);
CREATE INDEX idx_music_complaint_state ON music_complaint (state);
CREATE INDEX idx_music_complaint_music ON music_complaint (musicId);
INSERT INTO admin_menu_bar (name, ico, url, parentId, orderNum)
SELECT it.name, NULL, it.url, (SELECT parentId FROM admin_menu_bar WHERE url = '/admin/music_list.html'), it.orderNum
FROM (SELECT '歌曲纠错' AS name, '/admin/music_correct_list.html' AS url, 30 AS orderNum
      UNION ALL SELECT '歌曲投诉', '/admin/music_complaint_list.html', 40) it
WHERE EXISTS (SELECT 1 FROM admin_menu_bar WHERE url = '/admin/music_list.html')
  AND NOT EXISTS (SELECT 1 FROM admin_menu_bar m WHERE m.url = it.url);
INSERT OR IGNORE INTO admin_permission (roleId, menuId)
SELECT 1, id FROM admin_menu_bar WHERE url IN ('/admin/music_correct_list.html', '/admin/music_complaint_list.html');
";

/// 全新安装时的初始数据(菜单、超级管理员角色)
const INIT_DATA_FILE: &str = "init_data.sql";

/// 获取数据目录下的数据库文件路径
pub fn db_path(data_dir: &str) -> PathBuf {
    Path::new(data_dir).join(DB_FILE_NAME)
}

/// 全局数据库连接池
static DB_CONN: OnceLock<SqlitePool> = OnceLock::new();

/// 获取连接池
pub fn get() -> SqlitePool {
    DB_CONN.get().unwrap().clone()
}

/// 获取一个数据库操作上下文
pub fn get_context() -> DbContext {
    DbContext::new(DB_CONN.get().unwrap().clone())
}

/// 连接参数
fn connect_options(db_path: &Path) -> SqliteConnectOptions {
    //直接传文件路径,不拼接成URL,路径里有?、#等字符时也能正常打开
    SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        //WAL模式:多个读者,较少写者的场景(如Web服务器)
        .journal_mode(SqliteJournalMode::Wal)
        //数据库被锁时的等待时间
        .busy_timeout(Duration::from_secs(10))
}

/// 初始化数据库连接和表结构
/// - data_dir 数据目录,数据库文件保存在该目录下
pub async fn init(data_dir: &str) {
    //SQLite不会自动创建目录,先确保数据目录存在
    std::fs::create_dir_all(data_dir).unwrap();
    let db_path = db_path(data_dir);
    eprintln!("-->数据库初始化中...");
    init_pool(&db_path);
    upgrade(true).await.unwrap_or_else(|it| {
        eprintln!("-->数据库初始化失败: {}", it);
        std::process::exit(1);
    });
    eprintln!("-->数据库初始化完成。");
}

/// 在指定文件建库(建表并写入初始数据),数据迁移工具使用
pub async fn init_file(db_path: &Path) -> Result<(), Box<dyn Error>> {
    init_pool(db_path);
    upgrade(true).await
}

/// 编译期初始化数据库(仅建表),供build.rs生成DAO和sqlx宏检查sql语句使用
pub async fn init_build(db_path: &Path) {
    //每次都重新建库,保证表结构和ddl一致
    let _ = std::fs::remove_file(db_path);
    init_pool(db_path);
    upgrade(false).await.unwrap();
}

/// 初始化全局连接池
fn init_pool(db_path: &Path) {
    DB_CONN
        .set(
            SqlitePoolOptions::new()
                .max_connections(20)
                .connect_lazy_with(connect_options(db_path)),
        )
        .unwrap();
}

/// 数据库升级
/// - with_init_data 全新建库时是否写入初始数据(编译期的数据库不需要)
async fn upgrade(with_init_data: bool) -> Result<(), Box<dyn Error>> {
    let mut conn = get().acquire().await?;
    let version: i64 = sqlx::query_scalar("PRAGMA USER_VERSION")
        .fetch_one(&mut *conn)
        .await
        .unwrap_or(0);
    if version == 0 {
        //建表和初始数据在一个事务里完成,中途失败时下次启动重新建
        let mut tx = (&mut *conn).begin().await?;
        for sql_file in SQL_FILES {
            execute_sql_file(&mut tx, sql_file).await?;
        }
        if with_init_data {
            execute_sql_file(&mut tx, INIT_DATA_FILE).await?;
        }
        tx.commit().await?;
    } else {
        //每个版本的升级在一个事务里完成,中途失败时不会只升级一半
        if version < 2 {
            let mut tx = (&mut *conn).begin().await?;
            sqlx::raw_sql(UPGRADE_V2).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        if version < 3 {
            let mut tx = (&mut *conn).begin().await?;
            sqlx::raw_sql(UPGRADE_V3).execute(&mut *tx).await?;
            execute_sql_file(&mut tx, "music_video.sql").await?;
            tx.commit().await?;
        }
        if version < 4 {
            let mut tx = (&mut *conn).begin().await?;
            sqlx::raw_sql(UPGRADE_V4).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        if version < 5 {
            let mut tx = (&mut *conn).begin().await?;
            sqlx::raw_sql(UPGRADE_V5).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        if version < 6 {
            let mut tx = (&mut *conn).begin().await?;
            sqlx::raw_sql(UPGRADE_V6).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        if version < 7 {
            let mut tx = (&mut *conn).begin().await?;
            sqlx::raw_sql(UPGRADE_V7).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        if version < 8 {
            let mut tx = (&mut *conn).begin().await?;
            sqlx::raw_sql(UPGRADE_V8).execute(&mut *tx).await?;
            execute_sql_file(&mut tx, "vip_code.sql").await?;
            tx.commit().await?;
        }
        if version < 9 {
            let mut tx = (&mut *conn).begin().await?;
            sqlx::raw_sql(UPGRADE_V9).execute(&mut *tx).await?;
            tx.commit().await?;
        }
    }

    //设置数据库版本号(版本号没变时不写库)
    if version != VERSION as i64 {
        sqlx::query(&(format!("PRAGMA USER_VERSION = {}", VERSION)))
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// 执行一个建表文件
async fn execute_sql_file(conn: &mut SqliteConnection, sql_file: &str) -> Result<(), Box<dyn Error>> {
    let Some(sql_mbed) = Assets::get(sql_file) else {
        return Err(From::from(format!("{} not found", sql_file)));
    };
    let sql_str = std::str::from_utf8(sql_mbed.data.as_ref())?;
    sqlx::raw_sql(sql_str).execute(&mut *conn).await?;
    Ok(())
}
