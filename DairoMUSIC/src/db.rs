use sqlx_context::DbContext;

/// 开启一个事务,写数据时使用,写完之后必须调用commit(没有commit时自动回滚)
/// 用BEGIN IMMEDIATE开始事务,先拿到写锁,避免读了之后再写时因为别的连接在写而失败
pub async fn begin() -> Result<DbContext, sqlx::Error> {
    let mut ctx = lib_db::get_context();
    ctx.begin_immediate().await?;
    Ok(ctx)
}
