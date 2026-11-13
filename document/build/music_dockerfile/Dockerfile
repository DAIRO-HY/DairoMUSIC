#DairoMUSIC运行镜像
#程序在rust:1.99.1(Debian 13 trixie,glibc 2.41)里编译,运行环境也使用trixie,glibc版本一致
#用旧版本的Debian(比如bookworm的glibc 2.36)会因为缺少新版本glibc的函数而无法运行
FROM debian:trixie-slim

ARG TARGETARCH

#ffmpeg:读取歌曲信息、转码歌曲和视频(也可以在后台「依赖软件」里安装其他版本,安装了内置的会优先使用)
#tini:作为1号进程转发退出信号、回收僵尸进程(内置的DairoDFS、采集服务会启动子进程)
#procps:DairoMUSIC启动内置服务前用ps检查上次没有结束的进程
#xz-utils:后台安装依赖软件时解压tar.xz安装包
#ca-certificates:访问https(下载安装包、发送邮件)
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg tini procps xz-utils ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY ./build/$TARGETARCH/DairoMUSIC /app/DairoMUSIC

#授权可执行
RUN chmod +x DairoMUSIC

#解决控制台输出中文乱码
ENV LANG=C.UTF-8

EXPOSE 8020

#数据保存在/app/data(数据库、内置DFS和采集服务的程序及数据),请挂载到宿主机
ENTRYPOINT ["/usr/bin/tini", "--", "/app/DairoMUSIC", "--data-dir", "/app/data"]
