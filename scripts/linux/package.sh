#!/usr/bin/env bash
# 把 cargo 构建出的 Linux 可执行文件打成 AppImage 与 tar.gz（只能在 Linux x86_64 上运行）。
#
#   package.sh <可执行文件> <输出目录>
#   bash scripts/linux/package.sh target/release/mini-mqtt-client dist
#
# 产物（版本号只从根目录 Cargo.toml 的 [package] version 读）：
#   <输出目录>/mini-mqtt-client_<版本>_x86_64.AppImage
#   <输出目录>/mini-mqtt-client_<版本>_linux-x86_64.tar.gz（可执行文件、.desktop、图标、README、LICENSE）
#
# AppImage 目录（appimage.github.io）收录的自动测试有两条硬约束：
# - `.DirIcon` 必须存在且是 AppDir 内的**相对**软链（1.7.0 的 Tauri 包指向构建机绝对路径，挂载后断链被判 FATAL）
# - .desktop 要过 desktop-file-validate；目录里的分类与简介取自 Categories / Comment
# 所以 AppDir 根上的 .desktop / 图标 / .DirIcon / AppRun 全部用相对软链，打包前跑一次 desktop-file-validate（装了才跑）。
#
# 只打包可执行文件本身：GPUI 运行时用到的 libxkbcommon / libxcb / libwayland / libvulkan / fontconfig / freetype 是桌面
# 发行版的常备库，不随包分发。在 ubuntu-22.04 上构建，要求 glibc ≥ 2.35。
#
# appimagetool 默认下载 AppImage/appimagetool 的 continuous 版本；设 APPIMAGETOOL=<路径> 可改用本地已有的。
# 以 APPIMAGE_EXTRACT_AND_RUN=1 运行，不依赖 FUSE（CI runner 上没有）。
set -euo pipefail

die() {
  echo "::error::package.sh: $*" >&2
  exit 1
}

if [ "$#" -ne 2 ]; then
  echo "用法: $0 <可执行文件> <输出目录>" >&2
  exit 2
fi

BIN=$1
OUT_DIR=$2

[ "$(uname -s)" = "Linux" ] || die "只能在 Linux 上运行"
[ "$(uname -m)" = "x86_64" ] || die "目前只打 x86_64 包（当前 $(uname -m)）"
[ -x "$BIN" ] || die "找不到可执行文件 $BIN"
# 转成绝对路径：回读校验在 $WORK 里比对可执行文件，相对路径（release.yml 传的就是）cd 之后会找不到
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd "$SCRIPT_DIR/../.." && pwd)
CARGO_TOML="$ROOT_DIR/Cargo.toml"
DESKTOP_SRC="$SCRIPT_DIR/com.minimqtt.client.desktop"
ICON_SRC="$ROOT_DIR/assets/app-icon/icon.png"

EXE_NAME="mini-mqtt-client"
DESKTOP_NAME="com.minimqtt.client.desktop"
# 与 .desktop 的 Icon= 一致
ICON_NAME="mini-mqtt-client"
ICON_SIZE="512x512"

[ -f "$DESKTOP_SRC" ] || die "找不到 $DESKTOP_SRC"
[ -f "$ICON_SRC" ] || die "找不到图标 $ICON_SRC"

VERSION=$(awk -F'"' '/^\[package\]/{f=1;next} /^\[/{f=0} f && /^version *=/{print $2; exit}' "$CARGO_TOML")
[ -n "$VERSION" ] || die "读不到 $CARGO_TOML 的 [package] version"
echo "version: $VERSION"

if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "$DESKTOP_SRC"
else
  echo "::warning::没装 desktop-file-validate，跳过 .desktop 校验"
fi

mkdir -p "$OUT_DIR"
OUT_DIR=$(cd "$OUT_DIR" && pwd)
WORK=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/linux-pkg.XXXXXX")
trap 'rm -rf "$WORK"' EXIT

# ── AppImage ──────────────────────────────────────────────────────────
APPDIR="$WORK/AppDir"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/$ICON_SIZE/apps"
install -m 755 "$BIN" "$APPDIR/usr/bin/$EXE_NAME"
install -m 644 "$DESKTOP_SRC" "$APPDIR/usr/share/applications/$DESKTOP_NAME"
install -m 644 "$ICON_SRC" "$APPDIR/usr/share/icons/hicolor/$ICON_SIZE/apps/$ICON_NAME.png"
# 根目录的入口一律相对软链（见文件头）
ln -s "usr/share/applications/$DESKTOP_NAME" "$APPDIR/$DESKTOP_NAME"
ln -s "usr/share/icons/hicolor/$ICON_SIZE/apps/$ICON_NAME.png" "$APPDIR/$ICON_NAME.png"
ln -s "$ICON_NAME.png" "$APPDIR/.DirIcon"
ln -s "usr/bin/$EXE_NAME" "$APPDIR/AppRun"

for f in "$DESKTOP_NAME" "$ICON_NAME.png" .DirIcon AppRun; do
  target=$(readlink "$APPDIR/$f")
  case "$target" in
    /*) die "$f 是绝对路径软链（$target）" ;;
  esac
  [ -e "$APPDIR/$f" ] || die "$f -> $target 是断链"
done

TOOL="${APPIMAGETOOL:-}"
if [ -z "$TOOL" ]; then
  TOOL="$WORK/appimagetool-x86_64.AppImage"
  curl -fsSL --retry 3 --retry-delay 10 -o "$TOOL" \
    https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$TOOL"
fi

APPIMAGE="$OUT_DIR/${EXE_NAME}_${VERSION}_x86_64.AppImage"
rm -f "$APPIMAGE"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" "$APPDIR" "$APPIMAGE"
chmod +x "$APPIMAGE"

# 回读校验：解包后 .DirIcon 仍是指向包内文件的相对软链
(
  cd "$WORK"
  "$APPIMAGE" --appimage-extract >/dev/null
  dir_icon=$(readlink squashfs-root/.DirIcon)
  case "$dir_icon" in
    /*) die "AppImage 里的 .DirIcon 是绝对路径软链（$dir_icon）" ;;
  esac
  [ -e squashfs-root/.DirIcon ] || die "AppImage 里的 .DirIcon 是断链（$dir_icon）"
  cmp -s "$BIN" "squashfs-root/usr/bin/$EXE_NAME" || die "AppImage 里的可执行文件与 $BIN 不一致"
  rm -rf squashfs-root
)

# ── tar.gz ────────────────────────────────────────────────────────────
STAGE_NAME="${EXE_NAME}_${VERSION}_linux-x86_64"
STAGE="$WORK/$STAGE_NAME"
mkdir -p "$STAGE"
install -m 755 "$BIN" "$STAGE/$EXE_NAME"
install -m 644 "$DESKTOP_SRC" "$STAGE/$DESKTOP_NAME"
install -m 644 "$ICON_SRC" "$STAGE/$ICON_NAME.png"
install -m 644 "$ROOT_DIR/README.md" "$ROOT_DIR/LICENSE" "$STAGE/"
TARBALL="$OUT_DIR/$STAGE_NAME.tar.gz"
tar -C "$WORK" -czf "$TARBALL" "$STAGE_NAME"

ls -la "$APPIMAGE" "$TARBALL"
sha256sum "$APPIMAGE" "$TARBALL"
