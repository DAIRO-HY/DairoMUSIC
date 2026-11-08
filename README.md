# DairoMUSIC

# 1. 系统说明

DairoMUSIC是一个音乐服务端，为DairoMUSIC客户端提供曲库、歌单、搜索、在线播放等功能，并带有后台管理系统。

## 产品特色

### 1). 轻量级

使用Rust语言开发，只有一个可执行文件（后台管理页面已经打包在程序里），使用SQLite数据库，不需要另外安装数据库，占用内存低、CPU消耗低。

### 2). 文件存储使用DairoDFS

歌曲文件、封面图片都保存在[DairoDFS](https://github.com/DAIRO-HY/DairoDFS)，支持断点续传和在线播放。

### 3). 自动处理歌曲

会员上传的歌曲由后台自动读取歌曲信息、转码成不同音质（320k、128k），并上传到DairoDFS。<br>
曲库里已经存在相同的歌曲文件时，自动合并到已有的歌曲。

### 4). 歌曲采集（可选）

可以配合DairoMusicSearch采集服务，从采集平台搜索、采集歌曲。不需要时在后台关闭即可。

### 5). 后台管理

管理员、角色权限、导航菜单、会员、歌曲、重复歌曲去重、系统配置。

# 2. 准备

1. 部署[DairoDFS](https://github.com/DAIRO-HY/DairoDFS)，在DairoDFS的"我的"页面里生成API票据，并开启文件访问路径。
2. 安装[FFmpeg](https://ffmpeg.org/)（包括ffprobe），用于读取歌曲信息和转码。

# 3. 编译和运行

需要安装[Rust](https://www.rust-lang.org/)（stable版本）。

```bash
#开发时运行(数据保存在项目根目录的data目录下)
cargo run -p DairoMUSIC

#编译发布版本
cargo build --release -p DairoMUSIC

#运行
./target/release/DairoMUSIC --dfs-url http://127.0.0.1:8031 --dfs-token <DairoDFS的API票据>
```

# 4. 配置

## 1). 安装完成之后，请从浏览器打开[http://127.0.0.1:8020/admin/](http://127.0.0.1:8020/admin/)。<br>
※其中127.0.0.1是你的服务器ip地址，请根据实际情况输入，:8020是程序默认端口，如需更改端口，请参考启动参数。

## 2). 第一次打开时，请根据页面提示创建超级管理员账号。

## 3). 登录后台之后，在"系统设置 → 系统配置"里设置：

- 是否开放注册
- 邮件服务（发送注册、重置密码的验证码）
- FFmpeg路径（为空时使用系统PATH里的ffmpeg）
- 歌曲采集（可选）

# 5. 启动参数

| 参数 | 环境变量 | 默认值 | 说明 |
|---|---|---|---|
| `--web-port` | `DAIRO_MUSIC_WEB_PORT` | 8020 | 程序监听端口 |
| `--data-dir` | `DAIRO_MUSIC_DATA_DIR` | ./data | 数据保存目录，数据库和上传的临时文件都在这个目录下 |
| `--dfs-url` | `DAIRO_MUSIC_DFS_URL` | http://127.0.0.1:8031 | DairoDFS的地址（服务端调用，同一台机器或内网时用内网地址） |
| `--dfs-public-url` | `DAIRO_MUSIC_DFS_PUBLIC_URL` | 和`--dfs-url`相同 | DairoDFS对外的地址，客户端播放歌曲、显示图片时使用 |
| `--dfs-token` | `DAIRO_MUSIC_DFS_TOKEN` | | DairoDFS的API票据，建议通过环境变量传入 |
| `--mock-email` | `DAIRO_MUSIC_MOCK_EMAIL` | 不开启 | 模拟发送邮件，**只用于开发和测试**：验证码不发送邮件，只记录到日志，并且可以从本机通过`GET /api/dev/email_code?email=`获取 |

# 6. 文档

| 文档 | 说明 |
|---|---|
| [客户端接口](./document/client-api.md) | 客户端(App)调用的接口 |

# 7. 项目结构

| 目录 | 说明 |
|---|---|
| `DairoMUSIC/` | 服务端程序 |
| `DairoMUSIC/assets/ddl/` | 建表语句 |
| `DairoMUSIC/assets/dsl/` | 数据库操作(编译时生成DAO) |
| `DairoMUSIC/assets/resources/` | 后台管理页面、分享页、下载页 |
| `lib_db/` | 数据库初始化和升级 |

# License

[Apache License 2.0](./LICENSE)
