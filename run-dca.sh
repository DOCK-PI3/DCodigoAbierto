#!/usr/bin/env sh
set -eu

# DCA-IA-IMPROVEMENT: Launcher POSIX con instalacion de Rust, build condicional y ejecucion.
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$SCRIPT_DIR"

export PATH="$HOME/.cargo/bin:$PATH"
BINARY="target/release/dca"

ensure_cargo() {
    if command -v cargo >/dev/null 2>&1; then
        return 0
    fi

    echo "[DCA] Rust no esta instalado. Instalando..."

    if command -v curl >/dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- https://sh.rustup.rs | sh -s -- -y --profile minimal
    else
        echo "[DCA] Error: se necesita curl o wget para instalar Rust." >&2
        return 1
    fi

    export PATH="$HOME/.cargo/bin:$PATH"
    command -v cargo >/dev/null 2>&1
}

ensure_cargo

if [ ! -x "$BINARY" ]; then
    echo "[DCA] Binario no encontrado. Compilando en release..."
    cargo build --release --bin dca
fi

if [ "${DCA_SKIP_RUN:-0}" = "1" ]; then
    echo "[DCA] Validacion completada. Ejecucion omitida por DCA_SKIP_RUN=1."
    exit 0
fi

echo "[DCA] Ejecutando $BINARY"
exec "$BINARY" "$@"