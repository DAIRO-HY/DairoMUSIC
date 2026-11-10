-- 比指定版本号新的最新稳定版
-- @name check_update
-- @param client_flag:i64
-- @param version_code:i64
-- @return AppVersion?
select * from app_version where clientFlag = :client_flag and versionCode > :version_code and state = 1 and releaseType = 1
order by versionCode desc limit 1;
