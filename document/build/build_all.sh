#!/bin/bash

#遇到错误立即停止
set -e

# 进入项目根目录
cd "$(dirname "$0")/../.."

./document/build/build_music.sh
./document/build/build_music_cross.sh
./document/build/build_github.sh
./document/build/build_music_docker.sh
