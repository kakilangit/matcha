#!/bin/sh
# Installs the system packages needed to build and package Matcha on Debian/Ubuntu.
set -eu

sudo=
if [ "$(id -u)" -ne 0 ]; then
  sudo=sudo
fi

$sudo apt-get update
$sudo apt-get install -y --no-install-recommends \
  build-essential pkg-config cmake clang file \
  libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev \
  libfontconfig-dev libfreetype-dev libssl-dev libx11-xcb-dev libxcb1-dev \
  libzstd-dev libasound2-dev
