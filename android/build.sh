#!/usr/bin/env bash
# Build vizy as an Android app (.apk) for 64-bit ARM phones, with no Docker
# or Gradle: cargo builds the native library, then the standard Android SDK
# tools package it with miniquad's small Java activity.
#
# Needs JDK 17, and in $ANDROID_HOME (default ~/Android) the platform,
# build-tools and NDK versions below.
#
#   ./android/build.sh           build target/android/vizy.apk
#   ./android/build.sh install   ...and install + launch it with adb
set -euo pipefail
cd "$(dirname "$0")/.."

SDK=${ANDROID_HOME:-$HOME/Android}
NDK=$SDK/ndk/30.0.16248370
BUILD_TOOLS=$SDK/build-tools/37.0.0
ANDROID_JAR=$SDK/platforms/android-37.0/android.jar
ADB=$SDK/platform-tools/adb
MIN_SDK=26
TARGET_SDK=34
PACKAGE=io.github.d33pfield.vizy
TARGET=aarch64-linux-android
OUT=target/android
# Debug signing key, kept outside the repo.
KEYSTORE=$HOME/.android/vizy-debug.keystore

for need in "$NDK" "$BUILD_TOOLS" "$ANDROID_JAR"; do
    if [[ ! -e "$need" ]]; then
        cat >&2 <<EOF
Missing Android SDK packages (looked for $need).
Install them once, in a terminal so you can accept Google's license:

  $SDK/cmdline-tools/latest/bin/android --sdk=$SDK sdk install \\
      platforms/android-37.0 build-tools/37.0.0 ndk/30.0.16248370 platform-tools
EOF
        exit 1
    fi
done

version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
rm -rf "$OUT" && mkdir -p "$OUT"

echo "== native library"
toolchain=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$toolchain/aarch64-linux-android$MIN_SDK-clang
export CC_aarch64_linux_android=$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
export AR_aarch64_linux_android=$toolchain/llvm-ar
# Keep the build machine's home folder out of the binary (see release.sh).
export RUSTFLAGS="--remap-path-prefix=$HOME=~ \
--remap-path-prefix=$HOME/.cargo/registry/src=crates \
--remap-path-prefix=$PWD=vizy"
cargo rustc --lib --release --target "$TARGET" --crate-type cdylib
lib=target/$TARGET/release/libvizy.so

echo "== java activity"
miniquad=$(cargo metadata --format-version 1 | python3 -c '
import json, os, sys
m = json.load(sys.stdin)
print(os.path.dirname(next(p["manifest_path"] for p in m["packages"] if p["name"] == "miniquad")))')
python3 - "$miniquad/java" "$OUT/java" "$PACKAGE" <<'EOF'
import os, re, sys
src, out, package = sys.argv[1:]
# Code for miniquad's "//% HOOK" plugin points; unused hooks are dropped.
hooks = {
    # Keep the screen on while vizy is in front.
    "MAIN_ACTIVITY_ON_CREATE": "getWindow().addFlags(LayoutParams.FLAG_KEEP_SCREEN_ON);",
}
def fill(text):
    text = text.replace("TARGET_PACKAGE_NAME", package).replace("LIBRARY_NAME", "vizy")
    return re.sub(r"//% (\w+)", lambda m: hooks.get(m.group(1), ""), text)
for name, pkg in [("MainActivity.java", package), ("QuadNative.java", "quad_native")]:
    d = os.path.join(out, *pkg.split("."))
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(src, name)) as f, open(os.path.join(d, name), "w") as g:
        g.write(fill(f.read()))
EOF
javac -source 1.8 -target 1.8 -Xlint:-options -bootclasspath "$ANDROID_JAR" \
    -d "$OUT/classes" $(find "$OUT/java" -name '*.java')
mkdir -p "$OUT/dex"
"$BUILD_TOOLS/d8" --release --min-api $MIN_SDK --lib "$ANDROID_JAR" \
    --output "$OUT/dex" $(find "$OUT/classes" -name '*.class')

echo "== package"
cat > "$OUT/AndroidManifest.xml" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="$PACKAGE">
    <uses-feature android:glEsVersion="0x00020000" android:required="true" />
    <application android:label="vizy" android:hasCode="true" android:extractNativeLibs="true"
        android:theme="@android:style/Theme.Black.NoTitleBar.Fullscreen">
        <activity android:name=".MainActivity" android:exported="true"
            android:screenOrientation="sensorLandscape" android:launchMode="singleTask"
            android:configChanges="orientation|keyboardHidden|keyboard|screenSize|screenLayout|smallestScreenSize|uiMode|density">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
EOF
"$BUILD_TOOLS/aapt2" link -o "$OUT/base.apk" --manifest "$OUT/AndroidManifest.xml" \
    -I "$ANDROID_JAR" --min-sdk-version $MIN_SDK --target-sdk-version $TARGET_SDK \
    --version-code 1 --version-name "$version"
python3 - "$OUT/base.apk" "$OUT/dex/classes.dex" "$lib" <<'EOF'
import sys, zipfile
apk, dex, lib = sys.argv[1:]
with zipfile.ZipFile(apk, "a", zipfile.ZIP_DEFLATED) as z:
    z.write(dex, "classes.dex")
    z.write(lib, "lib/arm64-v8a/libvizy.so")
EOF
# 16 KB alignment for the native library, as current Android devices expect.
"$BUILD_TOOLS/zipalign" -P 16 -f 4 "$OUT/base.apk" "$OUT/aligned.apk"

if [[ ! -f "$KEYSTORE" ]]; then
    mkdir -p "$(dirname "$KEYSTORE")"
    keytool -genkeypair -keystore "$KEYSTORE" -alias vizy -keyalg RSA -keysize 2048 \
        -validity 10000 -storepass android -keypass android -dname "CN=vizy debug" >/dev/null
fi
"$BUILD_TOOLS/apksigner" sign --ks "$KEYSTORE" --ks-pass pass:android \
    --out "$OUT/vizy.apk" "$OUT/aligned.apk"
echo "built $OUT/vizy.apk"

if [[ "${1:-}" == install ]]; then
    "$ADB" install -r "$OUT/vizy.apk"
    "$ADB" shell am start -n "$PACKAGE/.MainActivity"
fi
