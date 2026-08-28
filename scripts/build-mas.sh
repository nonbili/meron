#!/usr/bin/env bash
# Build, sign and package Meron for the Mac App Store.
#
# The result is dist/Meron-mas.pkg, ready for scripts/upload-mac-app-store.ts.
#
# How this differs from scripts/build.sh (the Developer ID / DMG build):
#
#   * Universal. The App Store takes one binary for both architectures, so both
#     the Go app and the Rust sidecar are built for arm64 and x86_64 and lipo'd.
#   * The sidecar ships inside the bundle instead of embedded in the Go binary.
#     A sandboxed app may not exec something it wrote to its cache dir, and App
#     Review rejects executable code delivered outside the bundle either way, so
#     meron-core goes to Contents/MacOS/ and is signed as part of the app. The
#     app finds it there via bundledSidecarPath() — no embed_sidecar tag.
#   * Sandbox entitlements, and no disable-library-validation /
#     allow-unsigned-executable-memory (both are App Store rejections).
#   * Signed with Apple Distribution + a provisioning profile rather than
#     Developer ID, and packaged as a .pkg rather than a notarized .dmg.
#
# Required environment:
#   MAS_PROVISION_PROFILE   path to the macOS App Store provisioning profile
#                           (.provisionprofile) downloaded from the developer portal
# Optional environment:
#   APP_STORE_DEVELOPMENT_TEAM
#                           team ID, used to build the default signing identities
#   MAS_APP_IDENTITY        overrides "Apple Distribution: ... (TEAM)"
#   MAS_INSTALLER_IDENTITY  overrides "3rd Party Mac Developer Installer: ... (TEAM)"
#   MAS_BUILD_NUMBER        overrides the CFBundleVersion in
#                           desktop/build/darwin/Info.plist for this upload; every upload
#                           of a given version needs a higher one than the last
#   MAS_SKIP_BUILD=1        re-sign and re-package the app already in desktop/build/bin
#                           instead of rebuilding it
#   MAS_ARCHS               universal (default), arm64 or amd64. Building
#                           universal needs both Rust targets installed, which
#                           means rustup; without it this script re-execs itself
#                           inside `nix-shell`, which provides it.
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="Meron"
BUNDLE_ID="jp.nonbili.meron"
# Wails names the bundle after outputfilename ("meron"); the shipped app is
# "Meron.app", the same rename the DMG job does with ditto.
BUILT_APP="desktop/build/bin/meron.app"
STAGE_DIR="dist/mas"
APP_PATH="${STAGE_DIR}/${APP_NAME}.app"
PKG_PATH="dist/${APP_NAME}-mas.pkg"
MAS_ARCHS="${MAS_ARCHS:-universal}"

die() { echo "error: $*" >&2; exit 1; }

# ------------------------------------------------------------- nix-shell

# A universal sidecar needs both Rust targets installed, which means rustup.
# Rather than make the caller remember that, re-exec inside nix-shell (which
# provides it) when it is missing. The guard variable stops a second lap if
# that shell somehow still has no rustup, so cargo can fail with the advice
# below instead of looping.
if ! command -v rustup >/dev/null \
  && [ "${MERON_MAS_NIX_SHELL:-0}" != "1" ] \
  && command -v nix-shell >/dev/null \
  && [ -f shell.nix ]; then
  echo "==> rustup not found; re-running inside nix-shell"
  exec env MERON_MAS_NIX_SHELL=1 nix-shell --run "$(printf '%q ' "$0" "$@")"
fi

# ---------------------------------------------------------------- preflight

: "${MAS_PROVISION_PROFILE:?set MAS_PROVISION_PROFILE to the .provisionprofile downloaded from the developer portal}"
[ -f "$MAS_PROVISION_PROFILE" ] || die "provisioning profile not found: $MAS_PROVISION_PROFILE"

TEAM_ID="${APP_STORE_DEVELOPMENT_TEAM:-}"
APP_IDENTITY="${MAS_APP_IDENTITY:-}"
INSTALLER_IDENTITY="${MAS_INSTALLER_IDENTITY:-}"

if [ -z "$APP_IDENTITY" ]; then
  [ -n "$TEAM_ID" ] || die "set APP_STORE_DEVELOPMENT_TEAM, or MAS_APP_IDENTITY for the full identity name"
  APP_IDENTITY="Apple Distribution: Nonbili Inc. ($TEAM_ID)"
fi
if [ -z "$INSTALLER_IDENTITY" ]; then
  [ -n "$TEAM_ID" ] || die "set APP_STORE_DEVELOPMENT_TEAM, or MAS_INSTALLER_IDENTITY for the full identity name"
  INSTALLER_IDENTITY="3rd Party Mac Developer Installer: Nonbili Inc. ($TEAM_ID)"
fi

for identity in "$APP_IDENTITY" "$INSTALLER_IDENTITY"; do
  security find-identity -v | grep -qF "$identity" \
    || die "signing identity not in the keychain: $identity"
done

# Decode the profile up front so a profile for another app or distribution
# certificate fails locally instead of producing a late App Store upload error.
profile_plist="$(/usr/bin/mktemp -t meron-mas-profile)"
trap 'rm -f "$profile_plist"' EXIT
security cms -D -i "$MAS_PROVISION_PROFILE" > "$profile_plist" 2>/dev/null \
  || die "could not decode provisioning profile: $MAS_PROVISION_PROFILE"

profile_team_id="$(/usr/libexec/PlistBuddy -c 'Print :TeamIdentifier:0' "$profile_plist" 2>/dev/null || true)"
profile_app_id="$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:com.apple.application-identifier' "$profile_plist" 2>/dev/null || true)"
if [ -z "$profile_app_id" ]; then
  # Older profiles use the iOS-style key even for a Mac App Store profile.
  profile_app_id="$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:application-identifier' "$profile_plist" 2>/dev/null || true)"
fi
[ -n "$profile_team_id" ] && [ "$profile_app_id" = "$profile_team_id.$BUNDLE_ID" ] \
  || die "provisioning profile authorizes ${profile_app_id:-no App ID}, expected a team prefix followed by $BUNDLE_ID"

app_identity_sha="$(security find-identity -v | awk -v target="\"$APP_IDENTITY\"" \
  'substr($0, length($0) - length(target) + 1) == target { print $2; exit }')"
profile_certificate_sha="$(plutil -extract DeveloperCertificates.0 raw -o - "$profile_plist" \
  | /usr/bin/base64 -D | shasum -a 1 | awk '{ print $1 }')" \
  || die "could not read the distribution certificate from the provisioning profile"
[ "$(printf '%s' "$profile_certificate_sha" | tr '[:lower:]' '[:upper:]')" = "$app_identity_sha" ] \
  || die "provisioning profile does not authorize signing identity: $APP_IDENTITY"

command -v wails >/dev/null || die "wails not found; go install github.com/wailsapp/wails/v2/cmd/wails@v2.12.0"
command -v cargo >/dev/null || die "cargo not found"

VERSION="$(/usr/bin/plutil -extract info.productVersion raw desktop/wails.json 2>/dev/null \
  || python3 -c 'import json;print(json.load(open("desktop/wails.json"))["info"]["productVersion"])')"

# The build number is tracked separately from the marketing version, the way
# CURRENT_PROJECT_VERSION is on iOS, and lives in the Info.plist template that
# already owns CFBundleVersion. Read as text rather than with PlistBuddy: the
# file is a Go template, and the {{if}} blocks make it invalid plist until wails
# has rendered it.
default_build_number="$(awk '
  /<key>CFBundleVersion<\/key>/ { found = 1; next }
  found { gsub(/^[[:space:]]*<string>|<\/string>[[:space:]]*$/, ""); print; exit }
' desktop/build/darwin/Info.plist)"
case "${default_build_number:-}" in
  ''|*'{{'*) die "desktop/build/darwin/Info.plist has no literal CFBundleVersion to use as the
       build number; set one (or pass MAS_BUILD_NUMBER)" ;;
esac
BUILD_NUMBER="${MAS_BUILD_NUMBER:-$default_build_number}"

echo "==> Meron $VERSION (build $BUILD_NUMBER) for the Mac App Store"
echo "    app signing:       $APP_IDENTITY"
echo "    installer signing: $INSTALLER_IDENTITY"

# ----------------------------------------------------------------- sidecar

case "$MAS_ARCHS" in
  universal) rust_targets="aarch64-apple-darwin x86_64-apple-darwin"; wails_platform="darwin/universal" ;;
  arm64)     rust_targets="aarch64-apple-darwin";                     wails_platform="darwin/arm64" ;;
  amd64)     rust_targets="x86_64-apple-darwin";                      wails_platform="darwin/amd64" ;;
  *) die "MAS_ARCHS must be universal, arm64 or amd64 (got \"$MAS_ARCHS\")" ;;
esac

echo "==> Building the Rust core engine sidecar ($MAS_ARCHS)"
for target in $rust_targets; do
  if command -v rustup >/dev/null; then
    rustup target add "$target"
  fi
  cargo build --release --manifest-path meron-core/Cargo.toml --target "$target" || die \
    "the sidecar failed to build for $target. Without rustup, cargo can only build
       the host architecture — install rustup (or nix, so this script can find it
       in nix-shell), or build a single-architecture package with MAS_ARCHS=arm64.
       Note that an arm64-only package cannot be installed on Intel Macs."
done

mkdir -p desktop/build/sidecar
sidecar_out="desktop/build/sidecar/meron-core-mas"
# shellcheck disable=SC2086 # word splitting is how the target list is iterated
set -- $rust_targets
if [ $# -eq 2 ]; then
  lipo -create -output "$sidecar_out" \
    "meron-core/target/$1/release/meron-core" \
    "meron-core/target/$2/release/meron-core"
else
  cp "meron-core/target/$1/release/meron-core" "$sidecar_out"
fi
lipo -info "$sidecar_out"

# --------------------------------------------------------------------- app

# No embed_sidecar: the sidecar is staged into the bundle below instead, which
# also keeps the Go binary from carrying a second copy of it.
if [ "${MAS_SKIP_BUILD:-0}" = "1" ]; then
  echo "==> Reusing the existing $BUILT_APP (MAS_SKIP_BUILD=1)"
else
  echo "==> Building the Wails app ($MAS_ARCHS)"
  (cd desktop && wails build -clean -platform "$wails_platform")
fi

[ -d "$BUILT_APP" ] || die "wails did not produce $BUILT_APP"

# ------------------------------------------------------------- stage bundle

echo "==> Staging the bundle as ${APP_NAME}.app"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"
ditto "$BUILT_APP" "$APP_PATH"

echo "==> Staging the sidecar and provisioning profile into the bundle"
cp "$sidecar_out" "$APP_PATH/Contents/MacOS/meron-core"
chmod 0755 "$APP_PATH/Contents/MacOS/meron-core"
cp "$MAS_PROVISION_PROFILE" "$APP_PATH/Contents/embedded.provisionprofile"
# cp keeps the source mode, and a profile downloaded from the developer portal
# is usually 0600. App Store validation rejects a package containing anything
# only root can read, since that breaks signature verification at launch.
chmod 0644 "$APP_PATH/Contents/embedded.provisionprofile"

# CFBundleVersion is what App Store Connect dedupes uploads by, so it has to be
# bumpable without touching the marketing version in wails.json.
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $BUILD_NUMBER" "$APP_PATH/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$APP_PATH/Contents/Info.plist"

# Wails leaves the dev-time bits and any stale signature behind; the App Store
# rejects a bundle whose signature does not cover what is actually in it.
rm -rf "$APP_PATH/Contents/_CodeSignature"

# Keep the generated bundle aligned with the App ID validated from the
# provisioning profile above.
actual_id="$(/usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" "$APP_PATH/Contents/Info.plist")"
[ "$actual_id" = "$BUNDLE_ID" ] \
  || die "bundle identifier is $actual_id, expected $BUNDLE_ID"

# ------------------------------------------------------------------- sign

# The build runs long enough that the machine can slip into dark wake (awake for
# power or the network, display asleep) before signing starts, and macOS refuses
# to use a keychain private key there: signing dies with CSSMERR_CSP_IN_DARK_WAKE.
# `caffeinate -u` asserts user activity for as long as the command it wraps runs.
sign_awake() { /usr/bin/caffeinate -u -i "$@"; }

# Inside out: nested code first, then the bundle. The sidecar gets inherit-only
# entitlements so it adopts the app's sandbox at exec time.
#
# No --options runtime here: the hardened runtime is a Developer ID/notarization
# requirement, and the App Store neither wants nor needs it.
echo "==> Signing"
sign_awake codesign --force --timestamp \
  --entitlements desktop/build/darwin/EntitlementsMASHelper.plist \
  --sign "$APP_IDENTITY" \
  "$APP_PATH/Contents/MacOS/meron-core"

# Xcode injects the application identifier from the provisioning profile as it
# signs; a hand-rolled codesign does not. A bundle whose signature lacks
# com.apple.application-identifier while its profile carries one uploads fine
# but is not eligible for TestFlight (ITMS-90886), so add it here from the
# profile already validated above. It stays out of the checked-in plist
# because it embeds the team ID, which is configurable.
app_entitlements="$(/usr/bin/mktemp -t meron-mas-entitlements)"
trap 'rm -f "$profile_plist" "$app_entitlements"' EXIT
cp desktop/build/darwin/EntitlementsMAS.plist "$app_entitlements"
plist_put() {
  /usr/libexec/PlistBuddy -c "Set :$1 $2" "$app_entitlements" 2>/dev/null \
    || /usr/libexec/PlistBuddy -c "Add :$1 string $2" "$app_entitlements"
}
plist_put com.apple.application-identifier "$profile_app_id"
plist_put com.apple.developer.team-identifier "$profile_team_id"

# The sidecar keeps EntitlementsMASHelper.plist untouched: com.apple.security
# .inherit is only valid on its own, and it is the app bundle's signature that
# has to carry the identifier.
sign_awake codesign --force --timestamp \
  --entitlements "$app_entitlements" \
  --sign "$APP_IDENTITY" \
  "$APP_PATH"

codesign --verify --deep --strict --verbose=2 "$APP_PATH"

# A bundle that reaches App Store Connect unsandboxed comes back as a rejection
# email hours later, so fail here instead.
echo "==> Entitlements check"
signed_entitlements="$(codesign -d --entitlements - --xml "$APP_PATH" 2>/dev/null)"
printf '%s' "$signed_entitlements" | grep -q 'app-sandbox' \
  || die "the signed bundle has no app-sandbox entitlement"
printf '%s' "$signed_entitlements" | grep -q "$profile_app_id" \
  || die "the signed bundle has no com.apple.application-identifier of $profile_app_id,
       which makes the upload ineligible for TestFlight (ITMS-90886)"

# Same idea: App Store validation rejects a package holding anything only root
# can read, and that answer arrives minutes into an upload rather than here.
echo "==> Permission check"
unreadable="$(find "$APP_PATH" \
  \( \( -type d ! -perm -o+rx \) -o \( ! -type d ! -perm -o+r \) \) -print -quit)"
[ -z "$unreadable" ] \
  || die "not readable by non-root users, which App Store validation rejects: $unreadable"

# ------------------------------------------------------------------ package

echo "==> Building the installer package"
mkdir -p dist
rm -f "$PKG_PATH"
sign_awake productbuild --component "$APP_PATH" /Applications \
  --sign "$INSTALLER_IDENTITY" \
  "$PKG_PATH"

pkgutil --check-signature "$PKG_PATH"

echo
echo "==> Done: $PKG_PATH"
echo "    Upload it with: bun scripts/upload-mac-app-store.ts"
