#!/usr/bin/env bash
# Génère les bindings C# de dmx-ffi (uniffi-bindgen-cs v0.11.0+v0.31.0) et copie la bibliothèque
# native de la plateforme courante pour les tests des ViewModels.
#
# Installation de l'outil :
#   cargo install uniffi-bindgen-cs --git https://github.com/NordSecurity/uniffi-bindgen-cs --tag v0.11.0+v0.31.0
set -euo pipefail
source "$(dirname "$0")/env.sh"
cd "$DMX_ROOT"

PROFILE_DIR="${PROFILE_DIR:-debug}"
INTEROP="$DMX_ROOT/windows/src/DmxMoney.Interop"

case "$(uname -s)" in
    Darwin) LIB="libdmx_ffi.dylib"; RID="osx-$(uname -m | sed 's/x86_64/x64/')" ;;
    Linux) LIB="libdmx_ffi.so"; RID="linux-x64" ;;
    *) LIB="dmx_ffi.dll"; RID="win-x64" ;;
esac

if [[ "$PROFILE_DIR" == "release" ]]; then
    cargo build -p dmx-ffi --release
else
    cargo build -p dmx-ffi
fi

mkdir -p "$INTEROP/Generated" "$INTEROP/runtimes/$RID/native"
uniffi-bindgen-cs \
    --library "target/$PROFILE_DIR/$LIB" \
    --config core/crates/dmx-ffi/uniffi.toml \
    --out-dir "$INTEROP/Generated"

cp "target/$PROFILE_DIR/$LIB" "$INTEROP/runtimes/$RID/native/$LIB"
echo "==> Bindings C# dans $INTEROP/Generated, bibliothèque $RID copiée"
