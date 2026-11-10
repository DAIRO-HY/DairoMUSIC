use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

/// 生成密码哈希
/// 先转成MD5再用argon2加盐哈希:老系统里保存的是MD5,迁移数据时直接对MD5做argon2,用户不用重设密码
/// argon2比较耗CPU,放到阻塞线程里执行
pub async fn hash(pwd: &str) -> String {
    let md5 = format!("{:x}", md5::compute(pwd));
    tokio::task::spawn_blocking(move || hash_md5(&md5)).await.unwrap()
}

/// 对MD5格式的密码做argon2哈希(迁移老数据时使用)
pub fn hash_md5(md5: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(md5.as_bytes(), &salt)
        .unwrap()
        .to_string()
}

/// 验证密码是否正确
pub async fn verify(pwd: &str, hash: &str) -> bool {
    let md5 = format!("{:x}", md5::compute(pwd));
    let hash = hash.to_string();
    tokio::task::spawn_blocking(move || {
        let Ok(parsed) = PasswordHash::new(&hash) else {
            return false;
        };
        Argon2::default().verify_password(md5.as_bytes(), &parsed).is_ok()
    })
    .await
    .unwrap_or(false)
}
