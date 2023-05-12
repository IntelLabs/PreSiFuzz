#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

rm -rf build
rm -rf template
rm -rf output
rm -rf fusesoc*

fusesoc library add opentitan https://github.com/lowRISC/opentitan.git

cp ./Dockerfile ./fusesoc_libraries/opentitan/util/container/Dockerfile

if [[ "$(docker images -q opentitan 2> /dev/null)" == "" ]]; then
  cd ./fusesoc_libraries/opentitan
  docker build --network=host --build-arg proxy=$https_proxy -t opentitan -f ./util/container/Dockerfile .
  cd ../..
fi

docker run -t -i --net host \
  -v $(pwd)/../..:/home/dev/src \
  -v /usr/synopsys:/usr/synopsys \
  --env="DISPLAY" --volume="$HOME/.Xauthority:/root/.Xauthority:rw" \
  --env DEV_UID=$(id -u) --env DEV_GID=$(id -g) \
  --env USER_CONFIG=/home/dev/src/docker-user-config.sh \
  --env SNPSLMD_LICENSE_FILE="$SNPSLMD_LICENSE_FILE" \
  --env http_proxy="$http_proxy" \
  --env https_proxy="$https_proxy" \
  --env ftp_proxy="$ftp_proxy" \
  opentitan \
  bash

