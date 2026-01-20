#!/usr/bin/env bash
set -euo pipefail

# Raise inotify limits only if writable inside this container (else skip)
if [ -w /proc/sys/fs/inotify/max_user_watches ] && [ -w /proc/sys/fs/inotify/max_user_instances ] && [ -w /proc/sys/fs/inotify/max_queued_events ]; then
  sudo sysctl -w fs.inotify.max_user_watches=1048576 >/dev/null 2>&1 || true
  sudo sysctl -w fs.inotify.max_user_instances=2048    >/dev/null 2>&1 || true
  sudo sysctl -w fs.inotify.max_queued_events=32768    >/dev/null 2>&1 || true
fi

# --- Stabilize .NET/MSBuild named sockets & runtime dirs ---
sudo chmod 1777 /tmp || true
sudo mkdir -p /run/user/$(id -u) || true
sudo chown -R "$(id -u):$(id -g)" /run/user/$(id -u) || true
chmod 700 /run/user/$(id -u) || true

# Env for child processes
export XDG_RUNTIME_DIR=/run/user/$(id -u)
export DOTNET_CLI_HOME=/home/vscode
export DOTNET_EnableDiagnostics=0

# NuGet robustness & cache
export NUGET_PACKAGES=/home/vscode/.nuget/packages
export NUGET_HTTPRETRY=${NUGET_HTTPRETRY:-8}
export NUGET_TIMEOUT=${NUGET_TIMEOUT:-120}

# (Optional) avoid HTTP/2 quirks on some paths
export DOTNET_SYSTEM_NET_HTTP_SOCKETSHTTPHANDLER_HTTP2SUPPORT=${DOTNET_SYSTEM_NET_HTTP_SOCKETSHTTPHANDLER_HTTP2SUPPORT:-false}

# --- GitHub CLI authentication bootstrap ---
if [ -n "${GITHUB_TOKEN:-}" ]; then
  export GH_TOKEN="${GH_TOKEN:-$GITHUB_TOKEN}"
  if command -v gh >/dev/null 2>&1 && ! gh auth status >/dev/null 2>&1; then
    printf '%s' "$GITHUB_TOKEN" | gh auth login --with-token >/dev/null 2>&1 || true
  fi
fi
if command -v gh >/dev/null 2>&1; then
  gh repo set-default canuszczyk/codex >/dev/null 2>&1 || true
fi

APP_BOOTSTRAP="${APP_BOOTSTRAP:-}"     # e.g., "dotnet restore && npm ci"

log() {
  printf "\n\033[1;36m[setup]\033[0m %s\n" "$*"
}

ensure_claude_commands() {
  local commands_dir="/home/vscode/.claude/commands"
  local prompt_file="$commands_dir/port-release.md"
  local wiki_url="https://raw.githubusercontent.com/wiki/canuszczyk/codex/Port-Release-Prompt.md"

  # Fix ownership of .claude volume (may be root-owned when first created)
  [ -d /home/vscode/.claude ] && sudo chown -R "$(id -u):$(id -g)" /home/vscode/.claude || true

  # Create commands directory if it doesn't exist
  mkdir -p "$commands_dir"

  # Download port-release prompt from wiki if not present
  if [ ! -f "$prompt_file" ]; then
    log "Downloading port-release prompt from wiki..."
    if curl -fsSL "$wiki_url" -o "$prompt_file" 2>/dev/null; then
      log "port-release prompt installed to $prompt_file"
    else
      log "Warning: Could not download port-release prompt from wiki"
    fi
  else
    log "port-release prompt already present"
  fi
}

install_ai_clis() {
  # Skip with SKIP_AI_CLIS=1 for faster rebuilds
  if [[ "${SKIP_AI_CLIS:-0}" == "1" ]]; then
    log "Skipping AI CLI installation (SKIP_AI_CLIS=1)"
    return 0
  fi

  local npm_prefix="${NPM_CONFIG_PREFIX:-$HOME/.npm-global}"
  local bin_dir="$npm_prefix/bin"
  mkdir -p "$npm_prefix" "$bin_dir"
  npm config set prefix "$npm_prefix" >/dev/null 2>&1 || true

  # Ensure npm global bin and ~/.local/bin are in PATH for this session
  if [[ ":$PATH:" != *":$bin_dir:"* ]]; then
    export PATH="$bin_dir:$PATH"
  fi
  if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    export PATH="$HOME/.local/bin:$PATH"
  fi

  # Persist PATH additions to .bashrc for interactive shells
  local bashrc="$HOME/.bashrc"
  if [[ -f "$bashrc" ]]; then
    if ! grep -q '\.local/bin' "$bashrc" 2>/dev/null; then
      log "Adding ~/.local/bin to .bashrc"
      echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$bashrc"
    fi
    if ! grep -q '\.npm-global/bin' "$bashrc" 2>/dev/null; then
      log "Adding ~/.npm-global/bin to .bashrc"
      echo 'export PATH="$HOME/.npm-global/bin:$PATH"' >> "$bashrc"
    fi
  fi

  log "Ensuring AI CLIs (claude, gemini, codex, codexaw)… Set SKIP_AI_CLIS=1 to skip"

  # Claude: use official installer (installs to ~/.claude/local/bin)
  if ! command -v claude >/dev/null 2>&1; then
    log "Installing Claude CLI…"
    if timeout 120 bash -c 'curl -fsSL https://claude.ai/install.sh | bash'; then
      log "Claude CLI installed."
    else
      log "Warning: Failed/timeout installing Claude CLI (non-fatal)."
    fi
  fi

  # Gemini CLI via npm (timeout 120s)
  log "Installing Gemini CLI…"
  timeout 120 npm install -g @google/gemini-cli || log "Warning: Failed/timeout installing Gemini CLI (non-fatal)."

  # Codexaw (forked) - install first, then rename binary (timeout 120s)
  log "Installing Codexaw CLI…"
  if timeout 120 npm install -g https://github.com/canuszczyk/codex/releases/latest/download/codexaw.tgz; then
    if [ -x "$bin_dir/codex" ]; then
      mv "$bin_dir/codex" "$bin_dir/codexaw" >/dev/null 2>&1 || true
    fi
  else
    log "Warning: Failed/timeout installing codexaw (non-fatal)."
  fi

  # Official Codex (upstream) (timeout 120s)
  log "Installing Codex CLI…"
  timeout 120 npm install -g @openai/codex || log "Warning: Failed/timeout installing upstream codex (non-fatal)."
}

ensure_pnpm() {
  local pnpm_version="${PNPM_VERSION:-10.8.1}"

  # release_codexaw.sh expects pnpm at:
  #   /workspaces/codex/.local/lib/node_modules/pnpm/dist/pnpm.cjs
  # this script runs with CWD = workspace root, so .local is correct here
  if [ -f .local/lib/node_modules/pnpm/dist/pnpm.cjs ]; then
    return
  fi

  if ! command -v npm >/dev/null 2>&1; then
    log "npm not found; cannot install pnpm into .local"
    return
  fi

  log "Installing pnpm@${pnpm_version} into .local prefix for release_codexaw.sh"
  mkdir -p .local
  npm install -g "pnpm@${pnpm_version}" --prefix .local || true
}

bootstrap_app() {
  if [[ -n "$APP_BOOTSTRAP" ]]; then
    log "Running APP_BOOTSTRAP: $APP_BOOTSTRAP"
    bash -lc "$APP_BOOTSTRAP"
    return
  fi

  # Ensure dev caches are owned by the container user (for named volumes)
  for p in /home/vscode/.nuget /home/vscode/.npm /home/vscode/.codex; do
    [ -d "$p" ] && sudo chown -R "$(id -u):$(id -g)" "$p" || true
  done

  # ----- .NET deps -----
  if command -v dotnet >/dev/null 2>&1; then
    # Prefer a solution; otherwise pick dotnet-api; otherwise restore all projects
    sln="$(find . -maxdepth 3 -name '*.sln' | head -n1 || true)"
    if [[ -n "$sln" ]]; then
      log "dotnet restore ($(basename "$sln"))"
      if ( cd "$(dirname "$sln")" && dotnet restore ); then
        :
      else
        log "dotnet restore retry (disable parallel)"
        ( cd "$(dirname "$sln")" && dotnet restore --disable-parallel ) || true
      fi
    elif [[ -d dotnet-api ]]; then
      log "dotnet restore (dotnet-api/*)"
      ( cd dotnet-api && dotnet restore ) \
        || ( cd dotnet-api && dotnet restore --disable-parallel ) || true
    else
      csprojs=($(find . -name '*.csproj' | tr '\n' ' '))
      if (( ${#csprojs[@]} > 0 )); then
        log "dotnet restore (all projects)"
        dotnet restore "${csprojs[@]}" \
          || dotnet restore --disable-parallel "${csprojs[@]}" || true
      else
        log "skipping dotnet restore: no .sln or .csproj found"
      fi
    fi

    # Optional: compile flags that avoid named pipes for MSBuild/Roslyn
    export DOTNET_BUILD_FLAGS="-nodeReuse:false /p:UseSharedCompilation=false"
  fi

  # --- Install just ---
  if command -v cargo >/dev/null 2>&1; then
    if ! command -v just >/dev/null 2>&1; then
      log "Installing just"
      cargo install just || true
    else
      log "just is already installed"
    fi
  fi

  # --- Install cargo-nextest ---
  if command -v cargo >/dev/null 2>&1; then
    if ! command -v cargo-nextest >/dev/null 2>&1; then
      log "Installing cargo-nextest"
      cargo install cargo-nextest --locked || true
    else
      log "cargo-nextest is already installed"
    fi
  fi

  # ----- JS deps (npm, with optional workspaces) -----
  if command -v npm >/dev/null 2>&1; then
    if [[ -f package-lock.json || -f package.json ]]; then
      # decide if this repo actually uses npm workspaces at the root
      use_workspaces=false
      if [[ -f package.json ]] && grep -q '"workspaces"' package.json; then
        use_workspaces=true
      fi

      if [[ -f package-lock.json ]]; then
        if [[ "$use_workspaces" == true ]]; then
          log "npm ci (workspaces)"
          npm ci --workspaces --include-workspace-root || true
        else
          log "npm ci"
          npm ci || true
        fi
      else
        if [[ "$use_workspaces" == true ]]; then
          log "npm install (workspaces)"
          npm install --workspaces --include-workspace-root || true
        else
          log "npm install"
          npm install || true
        fi
      fi
    else
      log "skipping npm: no package.json at repo root"
    fi
  fi
}

main() {
  case "${1:-}" in
    --stop)
      exit 0
      ;;
    --quick)
      ensure_pnpm
      ensure_claude_commands
      install_ai_clis
      bootstrap_app
      log "Done."
      exit 0
      ;;
  esac

  ensure_pnpm
  ensure_claude_commands
  install_ai_clis
  bootstrap_app
  log "Done."
}

main "$@"
