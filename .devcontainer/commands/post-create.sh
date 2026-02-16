#!/bin/bash

# Add the current directory to the list of safe directories for Git
# to avoid warnings when using Git in the container
git config --global --add safe.directory $(pwd)

# Suspend Git's message about moving to `main` as the default branch name
git config --global init.defaultBranch master

echo "if [ ! -f ${CONTAINER_WORKSPACE_FOLDER}/.envrc ]; then touch ${CONTAINER_WORKSPACE_FOLDER}/.envrc; fi" >> ~/.bashrc

# Allow direnv to load environment variables every time a new shell is started
echo -e "\ndirenv allow ${CONTAINER_WORKSPACE_FOLDER}" >> ~/.bashrc