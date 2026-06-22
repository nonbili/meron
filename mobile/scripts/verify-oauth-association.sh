#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MOBILE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ANDROID_ASSETLINKS="${ANDROID_ASSETLINKS:-$MOBILE_DIR/oauth-association/android-assetlinks.template.json}"
APPLE_AASA="${APPLE_AASA:-$MOBILE_DIR/oauth-association/apple-app-site-association.template.json}"
ANDROID_PACKAGE="${ANDROID_PACKAGE:-jp.nonbili.meron}"
ANDROID_PATH_PREFIX="${ANDROID_PATH_PREFIX:-/meron/oauth}"
IOS_BUNDLE_ID="${IOS_BUNDLE_ID:-jp.nonbili.meron}"

usage() {
  cat <<EOF
Usage: mobile/scripts/verify-oauth-association.sh [options]

Validates local OAuth domain-association files before publishing them.
By default this checks the checked-in templates and allows placeholder values.

Options:
  --android-assetlinks PATH  assetlinks.json file to validate
  --apple-aasa PATH          apple-app-site-association file to validate
  --android-package ID       expected Android package (default: jp.nonbili.meron)
  --android-path-prefix PATH expected Android/iOS OAuth path prefix (default: /meron/oauth)
  --ios-bundle-id ID         expected iOS bundle id (default: jp.nonbili.meron)
  --production               fail if placeholder replacement values remain
  -h, --help                 show this help
EOF
}

PRODUCTION=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --android-assetlinks)
      ANDROID_ASSETLINKS="$2"
      shift
      ;;
    --apple-aasa)
      APPLE_AASA="$2"
      shift
      ;;
    --android-package)
      ANDROID_PACKAGE="$2"
      shift
      ;;
    --android-path-prefix)
      ANDROID_PATH_PREFIX="$2"
      shift
      ;;
    --ios-bundle-id)
      IOS_BUNDLE_ID="$2"
      shift
      ;;
    --production)
      PRODUCTION=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

python3 - "$ANDROID_ASSETLINKS" "$APPLE_AASA" "$ANDROID_PACKAGE" "$ANDROID_PATH_PREFIX" "$IOS_BUNDLE_ID" "$PRODUCTION" <<'PY'
import json
import pathlib
import re
import sys

assetlinks_path = pathlib.Path(sys.argv[1])
aasa_path = pathlib.Path(sys.argv[2])
android_package = sys.argv[3]
path_prefix = sys.argv[4]
ios_bundle_id = sys.argv[5]
production = sys.argv[6] == "1"

errors = []

try:
    assetlinks = json.loads(assetlinks_path.read_text())
except Exception as exc:
    errors.append(f"{assetlinks_path}: failed to parse JSON: {exc}")
    assetlinks = []

if not isinstance(assetlinks, list) or not assetlinks:
    errors.append(f"{assetlinks_path}: expected a non-empty JSON array")
else:
    target = assetlinks[0].get("target", {}) if isinstance(assetlinks[0], dict) else {}
    if target.get("namespace") != "android_app":
        errors.append(f"{assetlinks_path}: target.namespace must be android_app")
    if target.get("package_name") != android_package:
        errors.append(f"{assetlinks_path}: package_name must be {android_package}")
    fingerprints = target.get("sha256_cert_fingerprints", [])
    if not isinstance(fingerprints, list) or not fingerprints:
        errors.append(f"{assetlinks_path}: sha256_cert_fingerprints must be non-empty")
    for fingerprint in fingerprints:
        if fingerprint == "REPLACE_WITH_RELEASE_CERT_SHA256":
            if production:
                errors.append(f"{assetlinks_path}: replace REPLACE_WITH_RELEASE_CERT_SHA256")
            continue
        if not re.fullmatch(r"[0-9A-Fa-f]{2}(:[0-9A-Fa-f]{2}){31}", fingerprint):
            errors.append(f"{assetlinks_path}: invalid SHA-256 fingerprint {fingerprint!r}")

try:
    aasa = json.loads(aasa_path.read_text())
except Exception as exc:
    errors.append(f"{aasa_path}: failed to parse JSON: {exc}")
    aasa = {}

details = (
    aasa.get("applinks", {}).get("details", [])
    if isinstance(aasa, dict)
    else []
)
if not isinstance(details, list) or not details:
    errors.append(f"{aasa_path}: applinks.details must be non-empty")
else:
    app_ids = details[0].get("appIDs", []) if isinstance(details[0], dict) else []
    components = details[0].get("components", []) if isinstance(details[0], dict) else []
    if not isinstance(app_ids, list) or not app_ids:
        errors.append(f"{aasa_path}: appIDs must be non-empty")
    for app_id in app_ids:
        if app_id == f"REPLACE_WITH_TEAM_ID.{ios_bundle_id}":
            if production:
                errors.append(f"{aasa_path}: replace REPLACE_WITH_TEAM_ID")
            continue
        if not app_id.endswith(f".{ios_bundle_id}"):
            errors.append(f"{aasa_path}: appID {app_id!r} must end with .{ios_bundle_id}")
    if not any(
        isinstance(component, dict)
        and str(component.get("/", "")).startswith(path_prefix)
        for component in components
    ):
        errors.append(f"{aasa_path}: components must include path prefix {path_prefix}")

if errors:
    for error in errors:
        print(error, file=sys.stderr)
    sys.exit(1)

print("OAuth association files are valid.")
PY
