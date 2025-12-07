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
  for p in /home/vscode/.nuget /home/vscode/.npm; do
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
      bootstrap_app
      log "Done."
      exit 0
      ;;
  esac

  ensure_pnpm
  bootstrap_app
  log "Done."
}

main "$@"
