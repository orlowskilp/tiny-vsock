#!/bin/bash

# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Lukasz P. Orlowski <lukasz@orlowski.io>. All rights granted under MIT license.

set -e

RED="\e[0;31m"
GREEN="\e[0;32m"
YELLOW="\e[0;33m"
BLUE="\e[0;34m"
MAGENTA="\e[0;35m"
RESET="\e[0m"

PKG_VERSION=v$(grep '^version =' Cargo.toml | head -1 | awk -F'"' '{print $2}')
README_VERSION=v$(grep 'doc_version' README.md | head -1 | awk -F'-v' '{print $2}' | awk -F'-' '{print $1}')
if [[ "$PKG_VERSION" != "$README_VERSION" ]]; then
  echo -e "${RED}Version mismatch:${RESET}"
  {
  printf "${MAGENTA}%s${RESET} version:\t${BLUE}(%s)${RESET}\n" "Cargo.toml" "$PKG_VERSION"
  printf "${MAGENTA}%s${RESET} version:\t${BLUE}(%s)${RESET}\n" "README.md" "$README_VERSION"
} | column -s $'\t' -t
  exit 1
fi

TAG_REF_PREFIX="refs/tags/"
if [[ $(echo $GITHUB_REF | grep $TAG_REF_PREFIX | wc -l) -lt 1 ]]; then
  echo -e "The event is not a tag. Skipping git tag version check...\n"
  echo -e "${GREEN}Versions matching:${RESET}"
  {
  printf "${MAGENTA}%s${RESET} version:\t${BLUE}(%s)${RESET}\n" "Cargo.toml" "$PKG_VERSION"
  printf "${MAGENTA}%s${RESET} version:\t${BLUE}(%s)${RESET}\n" "README.md" "$README_VERSION"
} | column -s $'\t' -t
  exit 0
fi

TAG_VERSION=${GITHUB_REF#$TAG_REF_PREFIX}
if [[ "$PKG_VERSION" != "$TAG_VERSION" ]]; then
  echo -e "${RED}Version mismatch:${RESET}"
  {
  printf "${MAGENTA}%s${RESET} version:\t${BLUE}(%s)${RESET}\n" "Cargo.toml" "$PKG_VERSION"
  printf "${MAGENTA}%s${RESET} version:\t${BLUE}(%s)${RESET}\n" "git tag" "$TAG_VERSION"
} | column -s $'\t' -t
  exit 1
fi

echo -e "${GREEN}All versions matching:${RESET}"
{
  printf "${MAGENTA}%s${RESET} version:\t${BLUE}(%s)${RESET}\n" "Cargo.toml" "$PKG_VERSION"
  printf "${MAGENTA}%s${RESET} version:\t${BLUE}(%s)${RESET}\n" "README.md" "$README_VERSION"
  printf "${MAGENTA}%s${RESET} version:\t${BLUE}(%s)${RESET}\n" "git tag" "$TAG_VERSION"
} | column -s $'\t' -t