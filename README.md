# DairoMUSIC

# 1. 系统说明

DairoMUSIC是一个音乐服务端，为DairoMUSIC客户端提供曲库、歌单、搜索、在线播放等功能，并带有后台管理系统。

## 产品特色

### 1). 轻量级

使用Rust语言开发，只有一个可执行文件（后台管理页面已经打包在程序里），使用SQLite数据库，不需要另外安装数据库，占用内存低、CPU消耗低。

### 2). 文件存储使用DairoDFS

歌曲文件、封面图片都保存在[DairoDFS](https://github.com/DAIRO-HY/DairoDFS)，支持断点续传和在线播放。<br>
可以在后台直接安装内置的DairoDFS，由DairoMUSIC运行，第一次启动时自动初始化，不需要另外部署；也可以使用自己部署的DairoDFS。

### 3). 自动处理歌曲

会员上传的歌曲由后台自动读取歌曲信息、转码成不同音质（320k、128k），并上传到DairoDFS。<br>
曲库里已经存在相同的歌曲文件时，自动合并到已有的歌曲。

### 4). 歌曲采集（可选）

可以配合DairoMUSIC-Search采集服务，从采集平台搜索、采集歌曲。采集服务同样可以在后台安装成内置服务。不需要时在后台关闭即可。

### 5). 后台管理

管理员、角色权限、导航菜单、会员、歌曲（试听、审核、歌词）、重复歌曲去重、发现页（轮播图、推荐歌单分组）、系统配置，以及DairoDFS、采集服务和依赖软件（ffmpeg、ffprobe、yt-dlp、deno）的安装和更新。

# 2. 准备

只需要DairoMUSIC一个程序。DairoDFS、采集服务、FFmpeg等都可以在启动之后从后台安装（上传安装包，或者填写下载地址），不需要另外配置环境。

可以从[GitHub Releases](https://github.com/DAIRO-HY/DairoMUSIC/releases)下载各平台的可执行文件，或者使用Docker镜像`dairopapa/dairo-music`（参考[docker-compose.yml](./document/build/music_dockerfile/docker-compose.yml)）。<br>
※Linux版本需要glibc 2.39以上（Debian 13、Ubuntu 24.04及以上），旧系统请用Docker运行。

# 3. 编译和运行

需要安装[Rust](https://www.rust-lang.org/)（stable版本）。

```bash
#开发时运行(数据保存在项目根目录的data目录下)
cargo run -p DairoMUSIC

#编译发布版本
cargo build --release -p DairoMUSIC

#运行
./target/release/DairoMUSIC
```

# 4. 配置

## 1). 安装完成之后，请从浏览器打开[http://127.0.0.1:8020/admin/](http://127.0.0.1:8020/admin/)。<br>
※其中127.0.0.1是你的服务器ip地址，请根据实际情况输入，:8020是程序默认端口，如需更改端口，请参考启动参数。

## 2). 第一次打开时，请根据页面提示创建超级管理员账号。

## 3). 登录后台之后，在"系统设置"里设置：

- **DairoDFS配置**：
  - 选择内置DFS（推荐）或外部DFS。
  - 内置DFS：上传安装包或填写下载地址安装（"推荐地址"会从GitHub获取当前系统的最新版本）。端口、数据目录、存储目录、管理员账号都有默认值，不填就行。第一次启动时自动创建管理员并生成API票据。
  - 外部DFS：填写地址和API票据（在DairoDFS的"我的"页面里生成，并开启文件访问路径）。
- **采集服务配置**（可选）：启用歌曲采集，同样可以选择内置或外部。
- **依赖软件**：ffmpeg、ffprobe（读取歌曲信息、转码），yt-dlp、deno（内置采集服务使用）。安装了内置程序时优先使用内置的，否则使用填写的路径或系统PATH里的程序。重新安装就是更新版本。
- **系统配置**：是否开放注册、邮件服务（发送注册、重置密码的验证码）、视频等。

客户端访问DFS和采集服务的文件，默认通过DairoMUSIC代理（`/proxy/dairo-dfs/`、`/proxy/dairo-search/`），和访问DairoMUSIC使用同一个域名，只需要开放DairoMUSIC的端口。也可以设置成直接访问。<br>
内置服务的程序安装在`{数据目录}/bin`下，DairoMUSIC启动时一起启动，异常退出时自动重启，DairoMUSIC退出时一起停止。

# 5. 启动参数

| 参数 | 环境变量 | 默认值 | 说明 |
|---|---|---|---|
| `--web-port` | `DAIRO_MUSIC_WEB_PORT` | 8020 | 程序监听端口 |
| `--data-dir` | `DAIRO_MUSIC_DATA_DIR` | ./data | 数据保存目录，数据库和上传的临时文件都在这个目录下 |
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
