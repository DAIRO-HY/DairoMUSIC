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
{"serverId": "yxgqqjil9wcsyegvhrvetc1d7rehujy5", "version": "2.0.0", "allowRegister": true, "allowSaveAlbum": false, "allowDownloadMusic": true, "allowDownloadVideo": true, "videoEnable": true}
```
- `serverId`:服务器唯一标识,服务器第一次启动时生成,之后不会改变。服务器的域名、端口变了也不变,客户端可以用来区分不同服务器的缓存。
- `allowRegister` 为 false 时隐藏注册和"免注册使用"。
- `allowSaveAlbum`:是否允许把下载的视频保存到相册(后台"系统配置"里设置)。为 false 时下载视频不询问是否保存到相册。
- `allowDownloadMusic`:是否允许下载歌曲(后台"系统配置"里设置,默认允许)。为 false 时隐藏歌曲的下载按钮(歌曲详情的下载、歌单的全部下载),已经下载的歌曲仍可以播放和删除。下载时从歌曲的 `files` 里选择音质。
- `allowDownloadVideo`:是否允许下载视频(后台"系统配置"里设置,默认允许)。为 false 时隐藏视频的下载按钮,已经下载的视频仍可以播放和删除;`allowSaveAlbum` 只在允许下载视频时有意义。
- `videoEnable`:服务器是否开启了视频采集(后台启用歌曲采集,并且"系统配置"里开启了视频)。为 false 时服务器不会再采集和转码视频,`/api/music/video` 返回的采集中、转码中的视频会一直停在这个状态,客户端不要轮询,提示"该歌曲没有视频或服务器未开启视频采集功能";已经转码好的视频照常播放。

### GET /api/app_version/check
检查更新。参数:`clientFlag`(客户端标识,和登录时的编号一致)、`versionCode`(当前版本号)。没有新版本时返回 `null`。
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
  "videoState": 2,
  "playCount": 128,
  "favoriteId": 1,
  "files": [
    {
      "id": 203337, "md5": "文件的MD5,老数据可能为null",
      "bitRate": 128, "sampleRate": 44100, "dataStartPosition": null, "sampleCount": null, "frameLength": null,
      "size": 4560446, "format": "mp3", "url": "播放地址(支持断点续传)"
    },
    {
      "id": 203338, "md5": "...",
      "bitRate": 320, "sampleRate": 44100, "dataStartPosition": null, "sampleCount": null, "frameLength": null,
      "size": 11401115, "format": "mp3", "url": "播放地址(支持断点续传)"
    }
  ],
  "collect": {
    "platform": "YT", "extra": "采集参数", "url": "采集平台的播放地址",
    "videoUrl": "采集平台的视频预览地址", "videoInfoUrl": "视频预览的采集进度地址"
  }
}
```
- `artist`、`album` 可能为 null,由客户端决定显示什么。
- `favoriteId` 只有歌单里的歌曲才有。
- `playCount` 播放次数,只有曲库的搜索结果(`/api/music/search`)才有。
- `files` 是这首歌所有音质的文件,按 `bitRate` 从低到高排序;空数组表示还没有采集完成,可以用 `collect.url` 播放。
- 文件的 `id` 是文件ID,不会改变,可以用来做缓存的key(DairoDFS的地址变了也不影响)。
- `videoState` 视频状态:0 未采集,1 采集中,2 有视频,3 没有视频(YouTube上只有一张图片),4 后台删除了视频,5 采集失败。3、4 表示没有视频;0、1、5 还不确定,打开视频页时调用 `/api/music/video`。采集平台的搜索结果(`id` 为0)固定为0。
- `collect.videoUrl`、`collect.videoInfoUrl`:没有启用视频采集时为 null。见下面的"视频"。
- 由客户端按"播放音质"设置从 `files` 里选择文件:不超过设置的最高音质文件;所有文件都超过时用最低音质的文件。比特率999及以上为无损。最高音质就是 `files` 的最后一个。

| 接口 | 参数 | 说明 |
|---|---|---|
| `GET /api/music/search` | `key` | 从曲库搜索,最多30条,按匹配程度排序 |
| `GET /api/music/search_collect` | `key` | 从采集平台搜索,结果的 `id` 为0,播放用 `collect.url`;要加入歌单时先调用 `collect` |
| `GET /api/music/detail` | `id` | 歌曲详情 |
| `GET /api/music/video` | `id` | 歌曲视频,见下面的"视频" |
| `GET /api/music/lrc` | `id` | 歌词列表 `[{id, musicId, title, lrc, agreeCount, disagreeCount, state, source}]` |
| `POST /api/music/play` | `ids` | 提交播放记录,见下面的"播放次数" |
| `POST /api/music/collect` | `name`、`logo`、`platform`、`extra` | 采集平台的歌曲加入曲库,返回歌曲ID(已经加入过时直接返回) |
| `POST /api/music/correct` | `id`、`name`、`artist`、`album` | 纠错,只提交要修改的字段 |
| `POST /api/music/complaint` | `id`、`content` | 投诉 |
| `GET /api/music/complaint_samples` | | 投诉理由的选项 |
| `GET /api/music/upload/by_md5` | `md5` | 曲库里已经有这个文件时返回歌曲ID,没有时404 |
| `POST /api/music/upload` | `name`(带扩展名的文件名,URL参数);请求体直接放文件内容 | 上传本地歌曲(mp3、flac、wav、ape、m4a、aac、ogg、wma,500MB以内),返回歌曲ID。后台处理完成之前 `files` 为空数组 |

## 播放次数

一首歌**自然播放到结尾**算一次,调用 `POST /api/music/play` 让播放次数加1。

- `ids`:播放完成的歌曲ID,逗号分隔,同一首歌播放了几次就出现几次,例如 `12,12,35`。一次最多500个。
- 单曲循环时每播放完一遍都算一次。手动切歌、拖动进度条跳到结尾不算。
- 离线播放(播放已下载的歌曲)也要计数:播放完成时先保存到本地的待提交记录(要保存到磁盘,退出APP不能丢),然后提交,成功之后清除;提交失败(没有网络)时保留,下次播放完成、或者恢复网络、启动APP时再一起提交。在线和离线用同一套逻辑。
- 采集平台的搜索结果(`id` 为0,还没有加入曲库)不计,不需要提交。
- 不存在、已经删除的歌曲ID会被忽略,不会报错。接口返回成功就可以清除本地的记录。

## 视频

只有采集平台(YouTube)的歌曲有视频。服务器采集最高规格的原画(VP9、AV1等iPhone不能直接播放的编码转码成HEVC),原画的短边高于480或帧率高于25时,再转码一个480p 25fps的H.264流畅版(短边不到480的不放大,只降帧率)。所有视频都是mp4(faststart,可以边下边播),支持断点续传。

### GET /api/music/video?id=
曲库里的歌曲的视频。还没有采集过(老的歌曲)、或者采集失败超过1小时时,调用这个接口会开始采集,返回 `state=1`。采集、转码过程中每隔3秒调用一次,直到可以播放。
```json
{
  "state": 2,
  "original": {
    "id": 12, "quality": 1, "state": 1, "width": 3840, "height": 2160, "codec": "hevc",
    "bitRate": 14400, "frameRate": 30, "duration": 223488, "size": 402653184, "md5": "...", "url": "播放地址"
  },
  "smooth": {"id": 13, "quality": 2, "state": 0, "width": 0, "height": 0, "codec": null, "...": "...", "url": null}
}
```
- `state` 和歌曲的 `videoState` 相同。只有 `state=2` 时才有 `original`、`smooth`。
- 视频文件的 `state`:0 转码中,1 可以播放,2 转码失败。可以播放时才有 `url`。
- 先转码流畅版,再处理原画。`original`、`smooth` 在 `state=2` 时都不为 null;原画不高于480p 25fps时没有单独的流畅版,`smooth` 和 `original` 是同一个文件(`id` 相同)。
- 缓存、下载的key用视频文件的 `id`。

### 还没有收藏的歌曲(搜索结果)
采集平台的搜索结果(`id` 为0)只能播放预览版(iPhone能直接播放的最高规格,一般是1080p H.264),直接访问采集服务,不需要token:
1. `GET collect.videoInfoUrl` 返回纯文本:正好是 `OK` 表示可以播放;以"采集失败"开头表示失败;其他是进度文字。第一次请求时采集服务开始下载。
2. 返回 `OK` 后播放 `collect.videoUrl`。

采集服务的缓存保留24小时;加入曲库(`POST /api/music/collect`)时服务器同时开始采集视频。

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
| `DELETE /api/favorite/delete` | `id` | 删除歌单。首页(推荐分组、轮播)正在使用的歌单不会真正删除,只是不再属于这个会员(从"我的歌单"里消失,首页继续显示) |
| `POST /api/favorite/sort` | `ids`(排好序的歌单ID,逗号分隔) | 歌单排序 |
| `GET /api/favorite/musics` | `id` | 歌单里的歌曲,最近添加的在前面 |
| `GET /api/favorite/all_musics` | | 我所有歌单里的歌曲 |
| `POST /api/favorite/add_music` | `favoriteId`、`musicId` | 添加歌曲到歌单 |
| `DELETE /api/favorite/remove_music` | `favoriteId`、`musicId` | 从歌单删除歌曲 |

## 首页

### GET /api/home/swiper
轮播 `[{id, name, image, type, data}]`。`type`:1:推荐的歌单(`data`为歌单ID) 2:推荐单曲(`data`为歌曲ID) 3:拨打电话 4:跳转网页

### GET /api/home/useful
推荐歌单分组 `[{name, type, favorites:[歌单]}]`。`type`:1:表格显示 2:横向滚动。表格每行显示几个由客户端决定。
歌单是后台从会员的歌单里选出来的,由所属的会员维护:歌单的 `isMine` 为 true 时当前会员可以编辑,否则只能查看。

## 其他

| 接口 | 参数 | 说明 |
|---|---|---|
| `POST /api/feedback/add` | `content`、`clientFlag` | 意见反馈,返回给用户看的提示文字 |

## 公开页面(不需要登录)

| 地址 | 说明 |
|---|---|
| `/share/music.html?id=歌曲ID` | 歌曲分享页,可以在线播放和下载 |
| `/download.html` | App下载页 |
| `GET /api/public/music?id=` | 分享页使用的歌曲信息 |
| `GET /api/public/download_info` | 下载页使用的信息 |

## 开发测试

服务端以 `--mock-email` 启动时(只用于开发和测试),验证码不发送邮件,并且不限制同一个邮箱的发送间隔。自动测试可以从本机获取验证码:

### GET /api/dev/email_code
参数:`email`。返回这个邮箱最近一次发送的、还有效的验证码,没有时返回 `null`。
```json
{"type": "BIND_EMAIL", "code": "995314", "date": 1794057600000}
```
`type`:REGISTER:注册 RESET_PWD:重置密码 BIND_EMAIL:绑定邮箱。没有开启 `--mock-email`,或者不是从本机(127.0.0.1)访问时返回404。

## 和老版本(Kotlin版 /client/**)的区别

- 不再使用 `_token`、`_client`、`_versionName` 参数,登录票据放在请求头里,设备信息只在登录时提交。
- 不再需要 `set_login_info`,服务端校验登录票据时自动记录最后在线时间。
- 每台设备一个登录票据,可以多台设备同时登录。
- 密码不再由客户端先做MD5。
- 注册和重置密码分成两个接口;去掉了没有实现的手机号登录。
- 曲库搜索和采集平台搜索分成两个接口,客户端可以先显示曲库的结果。
- 歌曲统一为一种结构,不再把空的歌手、专辑替换成"未知"。
- 推荐歌单不再需要 `colNum` 参数和 `hashId`。
- 客户端编号统一为 `clientFlag`(0:WEB 1:Android 2:IOS 3:Windows 4:Mac 5:Linux),检查更新、意见反馈也使用这个编号(老版本是 1:Windows 2:Mac 3:Linux 4:Android 5:Ios)。
- 分享链接直接使用歌曲ID,去掉了 `decode_param`。
- 只能查看自己的歌单和推荐的歌单(老版本知道ID就能查看任何人的歌单)。
