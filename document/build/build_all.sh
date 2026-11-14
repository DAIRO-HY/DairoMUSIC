#!/bin/bash

#遇到错误立即停止
set -e

# 进入项目根目录
cd "$(dirname "$0")/../.."

#删除上次编译的文件,否则build目录里会一直留着以前版本的二进制文件
rm -rf ./build

./document/build/build_music.sh
./document/build/build_music_cross.sh
./document/build/build_github.sh
./document/build/build_music_docker.sh
