ARG VERSION=latest

# Version is provided in devcontainer.json
FROM archlinux:${VERSION}

# These are meant to be provided in devcontainer.json
ARG USERNAME

# These are the default values for the build arguments. Don't edit
# unless you know what you're doing.
ARG HOME_DIR=/home/${USERNAME}
ARG RUST_INSTALLER=install-rust.sh

# Install dependencies and clean up the cache
RUN pacman -Suy --disable-sandbox --noconfirm \
    pkg-config \
    git \
    gcc \
    aarch64-linux-gnu-gcc \
    musl \
    musl-aarch64 \
    make \
    jq \
    direnv \
    unzip \
    xxd \
    bash-completion \
    docker \
    docker-compose \
    sudo && \
    rm /var/cache/pacman/pkg/*

# Create a non-root user set it up and roll over to it
RUN useradd -ms /bin/bash ${USERNAME} && \
    usermod -aG docker ${USERNAME} && \
    echo "${USERNAME} ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

USER ${USERNAME}
WORKDIR ${HOME_DIR}

# Install Rust and some useful tools
ENV PATH=$PATH:${HOME_DIR}/.cargo/bin:${HOME_DIR}/.local/bin \
    CARGO_BIN=${HOME_DIR}/.cargo/bin
RUN echo eval '"$(direnv hook bash)"' >> ${HOME_DIR}/.bashrc
RUN curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y && \
    cargo install \
        dprint \
        cargo-llvm-cov && \
    rustup component add \
        rust-analysis \
        rust-analyzer
RUN curl -Os https://cli.codecov.io/latest/linux/codecov && \
    chmod +x codecov && \
    mv codecov ${CARGO_BIN}
RUN curl -sS https://starship.rs/install.sh | sh -s -- --yes && \
    starship init bash >> ~/.bashrc
RUN curl -fsSL https://claude.ai/install.sh | bash
