FROM mcr.microsoft.com/dotnet/sdk:8.0
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    sudo nano screen lsof curl gnupg wget ca-certificates apt-transport-https \
    software-properties-common procps locales iproute2 \
    python3 python3-pip python3-venv \
    openssh-client git git-lfs \
    pkg-config libssl-dev zstd && \
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
    npm install -g pnpm cross-env && \
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
ENV NPM_CONFIG_PREFIX=/home/${USERNAME}/.npm-global
ENV DOTNET_ROOT=/usr/share/dotnet
ENV PATH="/home/${USERNAME}/.npm-global/bin:${DOTNET_ROOT}:${PATH}:/home/${USERNAME}/.dotnet/tools:/home/${USERNAME}/.claude/local/bin"
ENV LANG=C.UTF-8 LANGUAGE=en_US:en LC_ALL=C.UTF-8
ENV DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1
ENV DOTNET_CLI_TELEMETRY_OPTOUT=1

# Install gitui (terminal git UI)
RUN ARCH=$(uname -m) && \
    case "$ARCH" in \
      x86_64) GITUI_FILE="gitui-linux-x86_64.tar.gz" ;; \
      aarch64) GITUI_FILE="gitui-linux-aarch64.tar.gz" ;; \
      *) echo "Unsupported arch: $ARCH"; exit 1 ;; \
    esac && \
    LATEST_URL=$(curl -sL https://api.github.com/repos/extrawurst/gitui/releases/latest \
      | grep "browser_download_url.*${GITUI_FILE}" \
      | cut -d '"' -f 4) && \
    curl -sL "$LATEST_URL" | tar xz -C /usr/local/bin && \
    chmod +x /usr/local/bin/gitui

# Install broot (terminal file manager)
RUN curl -sL "https://dystroy.org/broot/download/x86_64-linux/broot" -o /usr/local/bin/broot && \
    chmod +x /usr/local/bin/broot

# Symlink vi to nano (for tools like gitui that expect vi)
RUN ln -sf /usr/bin/nano /usr/local/bin/vi

USER $USERNAME

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && \
    ~/.cargo/bin/rustup component add clippy rustfmt && \
    ~/.cargo/bin/rustup target add aarch64-unknown-linux-musl

ENV PATH="/home/vscode/.cargo/bin:${PATH}"

# Set up user-local npm prefix for AI CLIs (allows auto-updates)
RUN mkdir -p ~/.npm-global && \
    npm config set prefix "$HOME/.npm-global"

# NOTE:
# AI CLIs (claude, gemini, codex, codexaw) are NOT installed at image build time.
# They are instead installed in .devcontainer/post-create-command.sh using the
# persisted npm cache volume. This makes "Rebuild Container" faster.

# Ensure common state directories exist (owned by vscode)
RUN mkdir -p ~/.codex/sessions ~/.claude ~/.gemini

WORKDIR /app
EXPOSE 3000 5432
SHELL ["/bin/bash", "-lc"]
CMD ["/bin/bash"]
