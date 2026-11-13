#!/bin/bash

#打包Docker镜像(amd64、arm64)并推送,需要先执行build_music.sh编译Linux版本

#遇到错误立即停止
set -e

# 进入项目根目录
cd "$(dirname "$0")/../.."

# 获取版本号
version=$(sed -n 's/^version = "\(.*\)"/\1/p' ./DairoMUSIC/Cargo.toml)

mkdir -p ./build/amd64
mkdir -p ./build/arm64
cp ./build/DairoMUSIC-linux-amd64-$version ./build/amd64/DairoMUSIC
cp ./build/DairoMUSIC-linux-arm64-$version ./build/arm64/DairoMUSIC

#docker用户名
docker_user=$DOCKER_USER
docker_pwd=$DOCKER_PASSWORD

#前提条件 安装 QEMU binfmt,目的:在amd64位linux系统上构建arm64位镜像(镜像里需要执行apt安装ffmpeg等)
docker run --privileged --rm tonistiigi/binfmt --install all

#---------------------------------------上传Docker镜像-----------------------------------------
echo "正在打包Docker镜像..."
cp ./document/build/music_dockerfile/Dockerfile ./

#构建时只需要build目录下的程序,避免把整个项目(包括target目录)发送给docker
printf '*\n!build/amd64\n!build/arm64\n' > .dockerignore

echo "$docker_pwd" | docker login -u $docker_user --password-stdin

#builder容器和构建过程中的RUN命令都不会继承docker的代理设置(比如Docker Desktop通过代理访问镜像仓库),需要手动传入
#否则拉取基础镜像会报network is unreachable,apt也无法联网
proxy=$(docker info --format '{{.HTTPSProxy}}')
proxy_opts=""
proxy_args=""
if [ -n "$proxy" ]; then
  [[ "$proxy" == *://* ]] || proxy="http://$proxy"
  proxy_opts="--driver-opt env.http_proxy=$proxy --driver-opt env.https_proxy=$proxy"

  #http_proxy、https_proxy是docker预定义的构建参数,只在构建时生效,不会保存到镜像里
  proxy_args="--build-arg http_proxy=$proxy --build-arg https_proxy=$proxy"
fi

#多平台构建需要使用docker-container驱动的builder
docker buildx inspect music-builder >/dev/null 2>&1 || docker buildx create --name music-builder --driver docker-container $proxy_opts
docker buildx inspect music-builder --bootstrap
docker buildx build --builder music-builder --platform linux/amd64,linux/arm64 $proxy_args \
  -t $docker_user/dairo-music:$version -t $docker_user/dairo-music:latest --push .
docker logout

rm Dockerfile .dockerignore

echo "---------------------------------------docker镜像推送完成--------------------------------------"
