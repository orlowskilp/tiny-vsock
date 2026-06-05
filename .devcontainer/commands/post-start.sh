#!/bin/bash

# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Lukasz P. Orlowski <lukasz@orlowski.io>. All rights granted under MIT license.

# Cheap trick to allow the container to access the host's Docker socket
DOCKER_SOCK=/var/run/docker.sock
sudo chmod 777 ${DOCKER_SOCK}

# Install Rust toolchains declared in rust-toolchain
rustup toolchain install