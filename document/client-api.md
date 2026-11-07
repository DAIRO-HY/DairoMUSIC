# DairoMUSIC 客户端接口

客户端(App)调用的接口。后台管理页面使用的 `/admin/api/**` 不在本文档内。

## 通用约定

- 所有客户端接口都在 `/api/` 下。读取用 `GET`,修改用 `POST`,删除用 `DELETE`。
- 参数可以放在 URL 参数里,也可以用 `application/x-www-form-urlencoded` 放在请求体里。参数名使用驼峰(`deviceId`),下划线写法(`device_id`)也可以。
- 除了标注"不需要登录"的接口,都需要在请求头里带上登录票据:`Authorization: Bearer <token>`。
- 成功:HTTP 200,响应体直接是 JSON(没有 `{code,msg,data}` 包装)。没有返回值的接口响应体为 `null`。
- 失败:按 HTTP 状态码区分
  | 状态码 | 说明 | 响应体 |
  |---|---|---|
  | 400 | 参数格式错误 | JSON `{"code":2,"msg":"表单验证失败","data":{"字段名":["错误信息"]}}` |
  | 401 | 没有登录、票据无效、账号已注销 | 空,客户端回到登录页 |
  | 403 | 账号被禁用、没有权限 | 文本 |
  | 404 | 数据不存在 | 空 |
  | 500 | 业务错误 | 一般是文本,直接显示给用户;`code=2` 的 JSON 表示表单字段错误,格式同400 |
- 时间都是毫秒时间戳;图片、歌曲文件都是可以直接访问的完整地址。
- 密码直接传明文(请使用 HTTPS),服务端加盐哈希保存。
- 客户端标识 `clientFlag`:0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux

## 公共(不需要登录)

### GET /api/common/config
服务器的公开配置,客户端选择服务器时调用,也可以用来判断地址是否是 DairoMUSIC 服务器。
```json
{"version": "2.0.0", "allowRegister": true}
```
`allowRegister` 为 false 时隐藏注册和"免注册使用"。

### GET /api/app_version/check
检查更新。参数:`client`(1:Windows 2:Mac 3:Linux 4:Android 5:Ios)、`versionCode`(当前版本号)。没有新版本时返回 `null`。
```json
{"versionName": "1.3.2", "versionCode": 4, "installType": 2, "instruction": "更新说明", "url": "下载地址"}
```
`installType`:1:手动安装 2:弹框提示,后台下载 3:强制更新,后台下载

### POST /api/email/send_code
发送邮箱验证码。参数:`email`、`type`(`register`:注册 `reset_pwd`:重置密码 `bind_email`:绑定邮箱)。
- 同一个邮箱1分钟内只能发送一次,验证码10分钟内有效,只能使用一次,输错5次作废。
- `reset_pwd` 时邮箱没有注册也返回成功(不发送邮件)。

## 账号

登录类接口(`temp_login`、`login`、`register`)都需要设备信息:`deviceId`(设备唯一标识,1~64位)、`clientFlag`、`clientVersion`(整数版本号)。同一台设备只保留一条登录记录。返回:
```json
{"token": "登录票据", "user": {会员信息}}
```
会员信息:
```json
{"id": 1, "email": "邮箱,临时会员为null", "nick": "昵称", "logo": "头像地址", "isTemp": false, "isFriend": false}
```
`isFriend` 为信赖用户,纠错直接生效。

| 接口 | 需要登录 | 参数 | 说明 |
|---|---|---|---|
| `POST /api/user/temp_login` | 否 | 设备信息 | 免注册使用,每台设备对应一个临时会员,同一台设备再次调用时找回同一个会员。关闭注册时不能创建新的临时会员 |
| `POST /api/user/login` | 否 | `email`、`pwd`、设备信息 | 邮箱登录。连续输错密码会被限制10分钟 |
| `POST /api/user/register` | 否 | `email`、`pwd`(6~32位)、`code`、设备信息 | 注册,注册后直接登录 |
| `POST /api/user/reset_pwd` | 否 | `email`、`code`、`pwd` | 重置密码,所有设备需要重新登录 |
| `POST /api/user/bind_email` | 是 | `email`、`code`、`pwd` | 临时会员绑定邮箱成为正式会员,歌单都保留。返回会员信息 |
| `GET /api/user/info` | 是 | | 会员信息 |
| `POST /api/user/edit` | 是 | `nick` | 修改昵称 |
| `POST /api/user/avatar` | 是 | multipart:`file` | 上传头像(JPG/PNG/WEBP,10MB以内),返回头像地址 |
| `POST /api/user/modify_pwd` | 是 | `oldPwd`、`pwd` | 修改密码,其他设备需要重新登录 |
| `POST /api/user/unregister` | 是 | `pwd` | 注销账号,注销后邮箱可以重新注册 |
| `DELETE /api/user/logout` | 是 | | 退出登录 |

## 歌曲

所有返回歌曲的接口都使用这个结构:
```json
{
  "id": 82659,
  "name": "晴天",
  "artist": "周杰伦",
  "album": "叶惠美",
  "logo": "封面地址",
  "duration": 269000,
  "maxBitRate": 320,
  "favoriteId": 1,
  "file": {
    "bitRate": 320, "sampleRate": 44100, "dataStartPosition": null, "sampleCount": null, "frameLength": null,
    "size": 11401115, "format": "mp3", "url": "播放地址(支持断点续传)"
  },
  "collect": {"platform": "YT", "extra": "采集参数", "url": "采集平台的播放地址"}
}
```
- `artist`、`album` 可能为 null,由客户端决定显示什么。
- `favoriteId` 只有歌单里的歌曲才有。
- `file` 为 null 表示还没有采集完成,可以用 `collect.url` 播放。
- 目标音质 `bitRate`(默认320):返回不超过这个音质的最高音质文件;所有文件都超过时返回最低音质的文件。比特率999及以上为无损。

| 接口 | 参数 | 说明 |
|---|---|---|
| `GET /api/music/search` | `key` | 从曲库搜索,最多30条,按匹配程度排序 |
| `GET /api/music/search_collect` | `key` | 从采集平台搜索,结果的 `id` 为0,播放用 `collect.url`;要加入歌单时先调用 `collect` |
| `GET /api/music/detail` | `id`、`bitRate` | 歌曲详情 |
| `GET /api/music/lrc` | `id` | 歌词列表 `[{id, musicId, title, lrc, agreeCount, disagreeCount, state, source}]` |
| `POST /api/music/collect` | `name`、`logo`、`platform`、`extra` | 采集平台的歌曲加入曲库,返回歌曲ID(已经加入过时直接返回) |
| `POST /api/music/correct` | `id`、`name`、`artist`、`album` | 纠错,只提交要修改的字段 |
| `POST /api/music/complaint` | `id`、`content` | 投诉 |
| `GET /api/music/complaint_samples` | | 投诉理由的选项 |
| `GET /api/music/upload/by_md5` | `md5` | 曲库里已经有这个文件时返回歌曲ID,没有时404 |
| `POST /api/music/upload` | `name`(带扩展名的文件名,URL参数);请求体直接放文件内容 | 上传本地歌曲(mp3、flac、wav、ape、m4a、aac、ogg、wma,500MB以内),返回歌曲ID。后台处理完成之前 `file` 为 null |

## 歌单

歌单结构:
```json
{"id": 1, "name": "我喜欢的音乐", "description": null, "logo": "封面地址", "isLike": true, "musicCount": 130, "isMine": true}
```
- `isLike` 为"我喜欢"歌单,不能删除、不能改名,`logo` 为 null,客户端显示固定的图标。
- 没有上传封面时,`logo` 自动使用最近添加的歌曲的封面。
- 只能查看自己的歌单,以及首页推荐、轮播里的歌单。

| 接口 | 参数 | 说明 |
|---|---|---|
| `GET /api/favorite/list` | | 我的歌单,按排序 |
| `GET /api/favorite/detail` | `id` | 歌单详情 |
| `POST /api/favorite/add` | `name` | 新建歌单,返回歌单ID |
| `POST /api/favorite/edit` | `id`、`name`、`description` | 修改歌单 |
| `POST /api/favorite/logo` | multipart:`id`、`file` | 上传封面,返回封面地址 |
| `DELETE /api/favorite/delete` | `id` | 删除歌单 |
| `POST /api/favorite/sort` | `ids`(排好序的歌单ID,逗号分隔) | 歌单排序 |
| `GET /api/favorite/musics` | `id`、`bitRate` | 歌单里的歌曲,最近添加的在前面 |
| `GET /api/favorite/all_musics` | `bitRate` | 我所有歌单里的歌曲 |
| `POST /api/favorite/add_music` | `favoriteId`、`musicId` | 添加歌曲到歌单 |
| `DELETE /api/favorite/remove_music` | `favoriteId`、`musicId` | 从歌单删除歌曲 |

## 首页

### GET /api/home/swiper
轮播 `[{id, name, image, type, data}]`。`type`:1:推荐的歌单(`data`为歌单ID) 2:推荐单曲(`data`为歌曲ID) 3:拨打电话 4:跳转网页

### GET /api/home/useful
推荐歌单分组 `[{name, type, favorites:[歌单]}]`。`type`:1:表格显示 2:横向滚动。表格每行显示几个由客户端决定。

## 其他

| 接口 | 参数 | 说明 |
|---|---|---|
| `POST /api/feedback/add` | `content`、`client` | 意见反馈,返回给用户看的提示文字 |

## 公开页面(不需要登录)

| 地址 | 说明 |
|---|---|
| `/share/music.html?id=歌曲ID` | 歌曲分享页,可以在线播放和下载 |
| `/download.html` | App下载页 |
| `GET /api/public/music?id=` | 分享页使用的歌曲信息 |
| `GET /api/public/download_info` | 下载页使用的信息 |

## 和老版本(Kotlin版 /client/**)的区别

- 不再使用 `_token`、`_client`、`_versionName` 参数,登录票据放在请求头里,设备信息只在登录时提交。
- 不再需要 `set_login_info`,服务端校验登录票据时自动记录最后在线时间。
- 每台设备一个登录票据,可以多台设备同时登录。
- 密码不再由客户端先做MD5。
- 注册和重置密码分成两个接口;去掉了没有实现的手机号登录。
- 曲库搜索和采集平台搜索分成两个接口,客户端可以先显示曲库的结果。
- 歌曲统一为一种结构,不再把空的歌手、专辑替换成"未知"。
- 推荐歌单不再需要 `colNum` 参数和 `hashId`。
- 分享链接直接使用歌曲ID,去掉了 `decode_param`。
- 只能查看自己的歌单和推荐的歌单(老版本知道ID就能查看任何人的歌单)。
