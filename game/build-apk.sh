#!/usr/bin/env bash
# Builds the Android APK for Nebula Drift 3D.
#
# This must be run on a machine with internet access to Google's Maven
# repository (dl.google.com) and an installed Android SDK — that
# combination is NOT available inside the sandbox that generated this
# project, which is why the APK isn't already sitting in this repo.
# See README-APK.md for full setup instructions.
set -euo pipefail
cd "$(dirname "$0")"

echo "== 1/4 Installing JS dependencies =="
npm install

echo "== 2/4 Building the web bundle =="
npm run build

echo "== 3/4 Syncing the Capacitor Android project =="
npx cap sync android

echo "== 4/4 Building the debug APK =="
cd android
./gradlew assembleDebug

APK_PATH="app/build/outputs/apk/debug/app-debug.apk"
if [ -f "$APK_PATH" ]; then
  echo ""
  echo "APK hazır: android/$APK_PATH"
else
  echo "Beklenen konumda APK bulunamadı, android/app/build/outputs/apk altına bakın."
fi
