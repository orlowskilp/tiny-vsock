#!/bin/bash

# Add the current directory to the list of safe directories for Git
# to avoid warnings when using Git in the container.
git config --global --add safe.directory $(pwd)

# Suspend Git's message about moving to `main` as the default branch name.
git config --global init.defaultBranch master

echo -e "\nsource /usr/share/bash-completion/bash_completion" >> ~/.bashrc

# Create `.envrc` if it doesn't exist, so that `direnv allow` doesn't complain.
echo -e "\nif [ ! -f ${CONTAINER_WORKSPACE_FOLDER}/.envrc ]; then touch ${CONTAINER_WORKSPACE_FOLDER}/.envrc; fi" >> ~/.bashrc

# Allow direnv to load environment variables every time a new shell is started
echo -e "\ndirenv allow ${CONTAINER_WORKSPACE_FOLDER}" >> ~/.bashrc

# Add an alias for the `claude` command with specific options
echo -e "\nalias claude='claude --model qwen3.6 --channels plugin:telegram@claude-plugins-official'" >> ~/.bashrc