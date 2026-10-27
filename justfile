set dotenv-load

cargo := env("HOME") + "/.cargo/bin/cargo"

default:
    @just --list

dev:
    pnpm tauri dev

debug:
    CHATNIP_HOME={{justfile_directory()}}/.debug CHATNIP_USE_DEBUG=true pnpm tauri dev

build:
    pnpm tauri build

check:
    {{cargo}} check --manifest-path src-tauri/Cargo.toml
    pnpm check

lint:
    pnpm lint

test:
    just test-rust
    just test-frontend

test-rust:
    cd src-tauri && {{cargo}} test --bin chatnip

test-frontend:
    pnpm exec vitest run

test-e2e *args:
    #!/usr/bin/env bash
    set -euo pipefail
    bin=$(lockf -k /tmp/chatnip-e2e-build.lock bash -c 'pnpm exec vite build >&2 && cd src-tauri && CARGO_TARGET_DIR=target/e2e {{cargo}} test --features webdriver,tauri/custom-protocol --test e2e --no-run --message-format=json-render-diagnostics' | jq -r 'select(.executable != null and .target.name == "e2e") | .executable')
    cd src-tauri && "$bin" --test-threads=1 {{args}}

test-all:
    just test
    just test-e2e

deps:
    pnpm install
    cd src-tauri && {{cargo}} fetch

release version:
    #!/usr/bin/env bash
    set -euo pipefail
    [[ "{{version}}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version is MAJOR.MINOR.PATCH, got {{version}}" >&2; exit 2; }
    git diff --quiet && git diff --cached --quiet || { echo "the tree has uncommitted changes" >&2; exit 1; }
    for f in package.json src-tauri/tauri.conf.json; do jq --arg v "{{version}}" '.version = $v' "$f" > "$f.next" && mv "$f.next" "$f"; done
    sed -i '' -e '/^\[package\]/,/^\[/ s/^version = ".*"/version = "{{version}}"/' src-tauri/Cargo.toml
    sed -i '' -e '/^name = "chatnip"$/{n;s/^version = ".*"/version = "{{version}}"/;}' src-tauri/Cargo.lock
    sed -i '' -e 's/^  version ".*"/  version "{{version}}"/' packaging/homebrew/Casks/chatnip.rb
    git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock packaging/homebrew/Casks/chatnip.rb
    git commit -m "Release {{version}}"
    git tag -a "v{{version}}" -m "Chatnip {{version}}"
    echo "tagged v{{version}}; git push && git push origin v{{version}} starts the release"
