-- 保存配置(不存在时添加)
-- @name save
-- @param name:String
-- @param value:String
insert into system_config (name, value) values (:name, :value) on conflict (name) do update set value = excluded.value;
