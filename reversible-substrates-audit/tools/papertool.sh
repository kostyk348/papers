# Shared by build.sh and verify_all.sh: build papertool once, return its path.
# The crate lives in ../../tooling/papertool; a sibling copy inside this package wins if
# present, so the package stays buildable on its own.
papertool_binary() {
  local here
  here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  local crate="${PAPERTOOL_CRATE:-$here/../tooling/papertool}"
  local bin="$crate/target/release/papertool.exe"
  if [ ! -x "$bin" ] && [ -x "$crate/target/release/papertool" ]; then
    bin="$crate/target/release/papertool"
  fi
  if [ ! -x "$bin" ]; then
    echo "[papertool] building $crate" >&2
    ( cd "$crate" && cargo build --release >&2 ) || return 1
  fi
  case "$bin" in
    /*) ;;
    *) bin="$here/$bin" ;;
  esac
  echo "$bin"
}
