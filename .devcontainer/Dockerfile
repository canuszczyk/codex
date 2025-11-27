FROM mcr.microsoft.com/dotnet/sdk:8.0
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    sudo nano screen lsof curl gnupg wget ca-certificates apt-transport-https \
    software-properties-common procps locales iproute2 \
    python3 python3-pip python3-venv \
    openssh-client git git-lfs \
    pkg-config libssl-dev && \
    git lfs install --system && \
    sed -i '/en_US.UTF-8/s/^# //g' /etc/locale.gen && \
    locale-gen en_US.UTF-8 && update-locale LANG=en_US.UTF-8 && \
    curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && \
    apt-get update && apt-get install -y nodejs && \
    curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
      | tee /usr/share/keyrings/githubcli-archive-keyring.gpg >/dev/null && \
    chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg && \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
      | tee /etc/apt/sources.list.d/github-cli.list >/dev/null && \
    apt-get update && apt-get install -y gh && \
    npm install -g pnpm cross-env @anthropic-ai/claude-code @openai/codex && \
    # preload github.com host key (ssh)
    ssh-keyscan -H github.com >> /etc/ssh/ssh_known_hosts 2>/dev/null || true && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

ARG USERNAME=vscode
ARG USER_UID=1000
ARG USER_GID=$USER_UID
RUN groupadd --gid $USER_GID $USERNAME && \
    useradd --uid $USER_UID --gid $USER_GID -m $USERNAME && \
    echo "$USERNAME ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# npm + dotnet cache envs
ENV NPM_CONFIG_CACHE=/home/${USERNAME}/.npm
ENV DOTNET_ROOT=/usr/share/dotnet
ENV PATH="${DOTNET_ROOT}:${PATH}:/home/${USERNAME}/.dotnet/tools"
ENV LANG=C.UTF-8 LANGUAGE=en_US:en LC_ALL=C.UTF-8
ENV DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1
ENV DOTNET_CLI_TELEMETRY_OPTOUT=1

USER $USERNAME

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && \
    ~/.cargo/bin/rustup component add clippy rustfmt && \
    ~/.cargo/bin/rustup target add aarch64-unknown-linux-musl

ENV PATH="/home/vscode/.cargo/bin:${PATH}"

WORKDIR /app
EXPOSE 3000 5432
SHELL ["/bin/bash", "-lc"]
CMD ["/bin/bash"]
