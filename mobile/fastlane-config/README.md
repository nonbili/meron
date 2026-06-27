# Fastlane: screenshots, framing & store uploads

Cross-platform release automation for the Meron mobile apps. Screenshots are
captured with [Maestro](https://maestro.mobile.dev/), framed with Fastlane
`frameit`, and uploaded to Google Play / App Store Connect with `supply` /
`deliver`.

All commands run from `mobile/`.

## Layout

- `fastlane/Fastfile` — imports the per-platform lanes below.
- `fastlane-config/android/{Appfile,Fastfile,Framefile.json}` — Play config + lanes.
- `fastlane-config/ios/{Appfile,Fastfile,Framefile.json}` — App Store config + lanes.
- `fastlane-config/assets/` — shared frame background + font.
- `fastlane/screenshots/<platform>/en-US/` — raw + framed captures (`android`, `ios`, `ipad`).
- `fastlane/metadata/<store>/en-US/` — listing text, changelogs, and framed screenshots that get uploaded.
- `.maestro/screenshots.yaml` — the capture flow (shared by every device).

## Setup

```sh
cd mobile
bundle install          # installs fastlane into vendor/bundle
# Maestro: curl -Ls "https://get.maestro.mobile.dev" | bash
```

## 1. Capture (Maestro)

Boot the simulator/emulator and sign in to an account first (the flow expects a
populated inbox), then:

```sh
./scripts/maestro-screenshots.sh android
./scripts/maestro-screenshots.sh ios   "iPhone 11 Pro Max"
./scripts/maestro-screenshots.sh ipad  "iPad Pro 12.9-inch"
```

Use a **frameit-supported** device size. App Store-accepted, frame-supported
choices: iPhone 11 Pro Max (6.5", 1242×2688) and iPad Pro 12.9-inch
(2048×2732). Newer iPhone Pro sizes have no frame yet.

## 2. Frame (frameit)

```sh
./scripts/frame-screenshots.sh android
./scripts/frame-screenshots.sh ios
./scripts/frame-screenshots.sh ipad
```

Framed images are copied into the metadata folders:
- Android → `fastlane/metadata/android/en-US/images/phoneScreenshots/`
- iOS + iPad → `fastlane/metadata/ios/en-US/screenshots/`

Edit the keyword/title text per screen in `fastlane-config/<platform>/Framefile.json`.

## 3. Credentials

- **Android** (`fastlane-config/android/Appfile` or env):
  `GOOGLE_PLAY_JSON_KEY` → Play service-account JSON path.
- **iOS** (App Store Connect API key, via env):
  `APP_STORE_KEY_ID`, `APP_STORE_ISSUER_ID`, `APP_STORE_KEY_FILEPATH` (.p8).

Upload lanes read release notes from the shared changelog `changelogs/v<version>.txt`
(version from `wails.json`): the bun scripts copy it into the Play changelog
(`<versionCode>.txt`) and the iOS `release_notes.txt` automatically.

The upload scripts are bun/TypeScript (run from anywhere — they resolve their own paths):

## 4. Upload screenshots only

```sh
mobile/scripts/upload-screenshots.ts android
mobile/scripts/upload-screenshots.ts ios     # uploads iPhone + iPad sizes
```

## 5. Build & upload binaries

```sh
# Android: builds a signed AAB then uploads to a track (default: production).
# Signing falls back to the shared NB_UPLOAD_* keystore in ~/.gradle/gradle.properties.
GOOGLE_PLAY_JSON_KEY=~/.gradle/nonbili-sa.json \
MERON_VERSION_CODE=2 \
  mobile/scripts/upload-google-play.ts production

# iOS: builds the IPA (xcodebuild via fastlane) then uploads to App Store Connect
APP_STORE_KEY_ID=… APP_STORE_ISSUER_ID=… APP_STORE_KEY_FILEPATH=~/.gradle/nonbili-as.p8 \
APP_STORE_DEVELOPMENT_TEAM=XXXXXXXXXX \
  mobile/scripts/upload-app-store.ts
```

Useful flags: `SKIP_BUILD=1` (upload an existing artifact), `IOS_SUBMIT_FOR_REVIEW=1`,
`GOOGLE_PLAY_TRACK=internal`, `GOOGLE_PLAY_UPLOAD_METADATA=1`.
