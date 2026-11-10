#!/usr/bin/env python3
"""Opt-in packaged-core SMTP/IMAP checks against ~/repo/mservers.

Run after building/installing the debug app and Android test APK (built with
-PtestRunner=jp.nonbili.meron.IsolatedCoreTestRunner), or after an
iOS build-for-testing. Credentials are read locally and never printed here.
"""
import argparse
import json
import os
import shlex
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=["android", "ios"])
    parser.add_argument("--derived-data", help="Xcode derived data containing a completed test build")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    accounts = json.loads((Path.home() / "repo/mservers/dashboard/data/accounts.json").read_text())
    account = accounts[3 if args.platform == "android" else 4]
    if args.platform == "android":
        for port in (3143, 3587):
            subprocess.run(["adb", "reverse", f"tcp:{port}", f"tcp:{port}"], check=True)
        result = subprocess.run([
            "adb", "shell", "am", "instrument", "-w",
            "-e", "class", "jp.nonbili.meron.MeronCoreNativeContractTest#openPgpLabSendAndDraft",
            "-e", "pgpLabEmail", shlex.quote(account["email"]),
            "-e", "pgpLabPassword", shlex.quote(account["password"]),
            "jp.nonbili.meron.test/jp.nonbili.meron.IsolatedCoreTestRunner",
        ], capture_output=True, text=True)
        output = (result.stdout + result.stderr).replace(account["password"], "[redacted]")
        print(output)
        if result.returncode or "OK (1 test)" not in output:
            raise SystemExit("Android OpenPGP lab failed")
    else:
        if not args.derived_data:
            parser.error("--derived-data is required for iOS")
        env = dict(os.environ)
        env["TEST_RUNNER_MERON_PGP_LAB_EMAIL"] = account["email"]
        env["TEST_RUNNER_MERON_PGP_LAB_PASSWORD"] = account["password"]
        subprocess.run([
            "xcodebuild", "test-without-building", "-quiet",
            "-project", str(root / "mobile/ios/Meron.xcodeproj"), "-scheme", "Meron",
            "-destination", "platform=iOS Simulator,name=iPhone 17",
            "-derivedDataPath", args.derived_data,
            "-only-testing:MeronTests/RustCoreBridgeContractTests/testOpenPgpLocalMailLab",
            "CODE_SIGNING_ALLOWED=NO",
        ], env=env, check=True)


if __name__ == "__main__":
    main()
