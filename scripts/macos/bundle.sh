#!/usr/bin/env bash
# 把 cargo 构建出的 mini-mqtt-client 可执行文件组装成 "Mini MQTT Client.app"，并对整个 bundle 做 ad-hoc 签名
# （只能在 macOS 上运行；写法参考 mini-todo 的 pc-gpui/scripts/macos/bundle.sh）。
#
#   bundle.sh <arm64 或单架构二进制> <x86_64 二进制|-> <输出目录>
#
#   # universal（release.yml / ci.yml）
#   bash scripts/macos/bundle.sh \
#     target/aarch64-apple-darwin/release/mini-mqtt-client \
#     target/x86_64-apple-darwin/release/mini-mqtt-client dist
#   # 单架构（本机试用）
#   bash scripts/macos/bundle.sh target/release/mini-mqtt-client - dist
#
# 产物：<输出目录>/Mini MQTT Client.app/Contents/{MacOS/mini-mqtt-client, Resources/icon.icns, Info.plist, _CodeSignature/}
#
# 身份与 1.x（Tauri 版）mac 包一致：CFBundleIdentifier = com.minimqtt.client（= src/backend/config.rs 的
# APP_IDENTIFIER），.app 名 / CFBundleName 都是 Mini MQTT Client —— 用户把新包拖进「应用程序」时直接替换旧版。
# - CFBundleShortVersionString / CFBundleVersion 按 Apple 规定只能是数字和点：取版本号 `-` / `+` 之前的部分
#   （2.0.0-beta.1 → 2.0.0），口径与 Windows 安装器的 VIProductVersion 相同；完整版本号由应用内设置页显示
# - 版本号只从根目录 Cargo.toml 的 [package] version 读（唯一来源）
#
# 签名：没有 Apple 开发者账号，最后一步 `codesign --force --sign -` 整包 ad-hoc 签名（封 Info.plist / Resources，
# 并重签 lipo 合并后的每个切片）。签完不要再改 bundle 里的任何文件。不开 hardened runtime、不打时间戳（只有公证才需要）。
set -euo pipefail

die() {
  echo "::error::bundle.sh: $*" >&2
  exit 1
}

if [ "$#" -ne 3 ]; then
  echo "用法: $0 <arm64 或单架构二进制> <x86_64 二进制|-> <输出目录>" >&2
  exit 2
fi

BIN_A=$1
BIN_B=$2
OUT_DIR=$3

[ "$(uname -s)" = "Darwin" ] || die "只能在 macOS 上运行（需要 lipo / plutil / codesign）"

# 路径按脚本自身位置解析：可以从任何目录调用
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd "$SCRIPT_DIR/../.." && pwd)
CARGO_TOML="$ROOT_DIR/Cargo.toml"
ICON="$ROOT_DIR/assets/app-icon/icon.icns"

BUNDLE_ID="com.minimqtt.client"
APP_NAME="Mini MQTT Client"
EXE_NAME="mini-mqtt-client"

[ -f "$CARGO_TOML" ] || die "找不到 $CARGO_TOML"
[ -f "$ICON" ] || die "找不到图标 $ICON"
[ -f "$BIN_A" ] || die "找不到可执行文件 $BIN_A"
if [ "$BIN_B" != "-" ]; then
  [ -f "$BIN_B" ] || die "找不到可执行文件 $BIN_B"
fi

VERSION=$(awk -F'"' '/^\[package\]/{f=1;next} /^\[/{f=0} f && /^version *=/{print $2; exit}' "$CARGO_TOML")
[ -n "$VERSION" ] || die "读不到 $CARGO_TOML 的 [package] version"
SHORT="${VERSION%%[-+]*}"
[[ "$SHORT" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]] || die "版本号 $VERSION 的数字部分 '$SHORT' 不是 1~3 段整数"
echo "version: $VERSION (CFBundleShortVersionString / CFBundleVersion = $SHORT)"

mkdir -p "$OUT_DIR"
OUT_DIR=$(cd "$OUT_DIR" && pwd)
APP="$OUT_DIR/$APP_NAME.app"
EXE="$APP/Contents/MacOS/$EXE_NAME"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

if [ "$BIN_B" = "-" ]; then
  cp "$BIN_A" "$EXE"
else
  lipo -create -output "$EXE" "$BIN_A" "$BIN_B"
fi
chmod 755 "$EXE"

ARCHS=$(lipo -archs "$EXE")
echo "archs: $ARCHS"
if [ "$BIN_B" != "-" ]; then
  for arch in arm64 x86_64; do
    case " $ARCHS " in
      *" $arch "*) ;;
      *) die "universal 二进制缺少 $arch 切片（lipo -archs: $ARCHS）" ;;
    esac
  done
fi

cp "$ICON" "$APP/Contents/Resources/icon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>${BUNDLE_ID}</string>
  <key>CFBundleExecutable</key>
  <string>${EXE_NAME}</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleIconFile</key>
  <string>icon.icns</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleShortVersionString</key>
  <string>${SHORT}</string>
  <key>CFBundleVersion</key>
  <string>${SHORT}</string>
  <key>CFBundleDevelopmentRegion</key>
  <string>zh_CN</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

plutil -lint "$APP/Contents/Info.plist"
# 身份自查：读回关键键（plutil -extract 读不到时报错退出）
for key in CFBundleIdentifier CFBundleExecutable CFBundleShortVersionString; do
  echo "Info.plist $key = $(plutil -extract "$key" raw -o - "$APP/Contents/Info.plist")"
done
[ "$(plutil -extract CFBundleIdentifier raw -o - "$APP/Contents/Info.plist")" = "$BUNDLE_ID" ] ||
  die "Info.plist 的 CFBundleIdentifier 不是 $BUNDLE_ID"

# ad-hoc 签名整个 bundle（最后一步）
codesign --force --sign - --timestamp=none "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv --verbose=4 "$APP" 2>&1 | grep -E '^(Identifier|Format|CodeDirectory|Signature|Sealed Resources|Info.plist)' || true

echo "bundle: $APP"
