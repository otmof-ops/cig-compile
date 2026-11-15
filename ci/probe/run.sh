#!/usr/bin/env bash
# One-off measurement on a hosted runner: does the glibc build of cig run its
# VM slower than the static musl build with mimalloc, and if it does, is the
# difference the page size its memory gets?
#
# Builds four flavors of cig 1.24.0 from one tree (flavor-patch.py makes the
# allocator a feature), a stage b program per benchmark and flavor, and times
# them interleaved (bench.py). Nothing here is part of the product.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
work=${RUNNER_TEMP:-/tmp}/probe-gap
rm -rf "$work"; mkdir -p "$work/home"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_TERM_COLOR=never
export CC_x86_64_unknown_linux_musl=musl-gcc

# cig and cigcompile never see the runner's home.
cigenv () {
  env HOME="$work/home" CIGSCRIPT_HOME="$work/home/.cigscript" CIGO_HOME="$work/home/.cigo" \
      XDG_CONFIG_HOME="$work/home/.config" XDG_DATA_HOME="$work/home/.local/share" \
      XDG_STATE_HOME="$work/home/.local/state" XDG_CACHE_HOME="$work/home/.cache" \
      CIG_SEAL_KEY=file NO_COLOR=1 CIG_DOCTOR=0 CIG_REPORTS=never CIG_MAX_STEPS=0 "$@"
}

echo "== conditions"
lscpu | grep -E "Model name|^CPU\(s\)|Hypervisor|Thread\(s\)|L1d|L2 |L3 " || true
echo "kernel $(uname -r)"
echo "THP enabled: $(cat /sys/kernel/mm/transparent_hugepage/enabled)"
echo "THP defrag:  $(cat /sys/kernel/mm/transparent_hugepage/defrag)"
ldd --version 2>&1 | sed -n 1p
free -m | sed -n 1,2p

echo "== musl C toolchain"
if ! command -v musl-gcc >/dev/null; then
  sudo apt-get -o Acquire::Retries=3 update -qq
  sudo apt-get -o Acquire::Retries=3 install -y -qq --no-install-recommends musl-tools
fi
musl-gcc --version 2>&1 | sed -n 1p

echo "== cig 1.24.0's tree"
git clone --quiet --depth 1 --branch v1.24.0 https://github.com/otmof-ops/cig-script "$work/cig-script"
git -C "$work/cig-script" log -1 --format='%H %s' | cut -c1-110
cd "$work/cig-script"
rustup show active-toolchain || rustup toolchain install
rustup target add x86_64-unknown-linux-musl
rustc -V
python3 -I "$here/flavor-patch.py" "$work/cig-script"

build_one () {  # NAME TARGET [cargo arguments]
  local name=$1 target=$2 t0=$SECONDS; shift 2
  echo "-- build $name ($target) $*"
  CARGO_TARGET_DIR="$work/target-$name" cargo build --release --locked --bin cig --target "$target" "$@" 2>&1 \
    | grep -E "Compiling (cigscript|libmimalloc-sys)|Finished|^error|^warning: unused" | tail -12
  echo "   $((SECONDS - t0)) s"
}
bin_of () {
  case $1 in
    gnu|gnu-mi) echo "$work/target-$1/x86_64-unknown-linux-gnu/release/cig" ;;
    *)          echo "$work/target-$1/x86_64-unknown-linux-musl/release/cig" ;;
  esac
}
build_one gnu     x86_64-unknown-linux-gnu
build_one gnu-mi  x86_64-unknown-linux-gnu  --features mi
build_one musl-mi x86_64-unknown-linux-musl --features mi
build_one musl    x86_64-unknown-linux-musl

echo "== the four binaries"
for f in gnu gnu-mi musl-mi musl; do
  b=$(bin_of "$f")
  test -x "$b"
  printf '%-8s %s  %s bytes  %s\n' "$f" "$(cigenv "$b" --version)" "$(stat -c %s "$b")" "$(file -b "$b" | cut -c1-70)"
done

echo "== cigcompile (debug) from this branch"
cd "$repo"
rustup show active-toolchain || rustup toolchain install
CARGO_TARGET_DIR="$work/target-cc" cargo build --locked --bin cigcompile 2>&1 \
  | grep -E "Compiling (cigscript|cigcompile)|Finished|^error" | tail -6
CC=$work/target-cc/debug/cigcompile
cigenv "$CC" --version

echo "== stage b programs, one set per flavor"
set_dir=$work/cig-script/bench/compute
for f in gnu gnu-mi musl-mi musl; do
  dst=$work/bin-$f; mkdir -p "$dst"; built=0
  for src in "$set_dir"/*.cig; do
    n=$(basename "$src" .cig)
    case $n in hello|startup-*) continue ;; esac
    cp "$src" "$dst/"
    if (cd "$dst" && cigenv CIGCOMPILE_RUNTIME="$(bin_of "$f")" CIGCOMPILE_OFFLINE=1 \
          "$CC" build "$n.cig" --stage b -o "$n.b" >"$n.build.log" 2>&1); then
      built=$((built + 1))
    else
      echo "STAGE B FAILED $f/$n"; tail -5 "$dst/$n.build.log" || true
    fi
  done
  echo "stage b $f: $built built"
done

echo "== timing"
cigenv python3 -I "$here/bench.py" "$work" "$set_dir" "${PROBE_ROUNDS:-11}"
