#!/bin/sh
# Builds the browser version (docs/web.md) into web/: the page, the
# wasm-bindgen glue and the module. Needs the wasm32-unknown-unknown
# target and the wasm-bindgen CLI of the version the crates pin
# (crates/widget/Cargo.toml):
#
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.129
#
#   tools/build-web.sh            # release build into web/
#   tools/build-web.sh --debug    # debug build (slow editor, quick build)
#   tools/build-web.sh --out DIR  # somewhere else
#
# Then serve the directory over HTTP, e.g.
#   python3 -m http.server -d web 8000
# and open http://localhost:8000/.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
out="$root/web"
profile=release
while [ $# -gt 0 ]; do
    case "$1" in
        --debug) profile=debug ;;
        --out) out=$2; shift ;;
        *) echo "usage: $0 [--debug] [--out DIR]" >&2; exit 2 ;;
    esac
    shift
done

bindgen=${WASM_BINDGEN:-wasm-bindgen}
if ! command -v "$bindgen" >/dev/null 2>&1 && [ -x "$HOME/.cargo/bin/wasm-bindgen" ]; then
    bindgen=$HOME/.cargo/bin/wasm-bindgen
fi
want=$(sed -n 's/^wasm-bindgen = "=\(.*\)"$/\1/p' "$root/crates/widget/Cargo.toml")
have=$("$bindgen" --version | cut -d' ' -f2)
if [ "$want" != "$have" ]; then
    echo "wasm-bindgen $have, but the crates need $want: cargo install wasm-bindgen-cli --version $want" >&2
    exit 1
fi

flag=
[ "$profile" = release ] && flag=--release
cargo build --manifest-path "$root/Cargo.toml" -p widget --target wasm32-unknown-unknown $flag

mkdir -p "$out"
rm -rf "$out/snippets"
"$bindgen" --target web --no-typescript --out-dir "$out" --out-name widget \
    "$root/target/wasm32-unknown-unknown/$profile/widget.wasm"
cp "$root/crates/widget/web/index.html" "$out/index.html"
cp "$root/crates/widget/res/icon.png" "$out/icon.png"
echo "built $out: serve it over HTTP (python3 -m http.server -d $out 8000)"
