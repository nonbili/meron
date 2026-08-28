#!/usr/bin/env bun

// Build and upload Meron for the Mac App Store.
//
// Runs scripts/build-mas.sh (pass SKIP_BUILD=1 to upload an existing .pkg),
// then hands the result to fastlane with the same App Store Connect API key the
// iOS lane uses. The build is a shell script rather than a fastlane lane
// because the desktop app is a Wails/Go build, not an Xcode project.
//
// Run this from a plain shell: build-mas.sh enters nix-shell on its own if it
// needs rustup for the universal sidecar. Do not wrap this in nix-shell
// yourself — fastlane and altool need the Apple toolchain, not nix's.

import { resolve } from 'node:path'
import {
  commandExists,
  copyFile,
  ensureFile,
  envFlag,
  fail,
  mobileDir,
  repoRoot,
  requireEnv,
  run,
} from '../mobile/scripts/release-utils'

const [pkgArg] = Bun.argv.slice(2)

// wails.json is the source of truth for the desktop version, the way
// Version.xcconfig is for iOS.
const wails = await Bun.file(resolve(repoRoot, 'desktop/wails.json')).json()
const version: string = wails.info.productVersion

// Whatever scripts/build-mas.sh writes; only meaningful to override alongside
// SKIP_BUILD, since the build script's output path is fixed.
const defaultPkgPath = resolve(repoRoot, 'dist/Meron-mas.pkg')
const pkgPath = pkgArg ?? process.env.PKG_PATH ?? defaultPkgPath
const appIdentifier = process.env.MAC_APP_IDENTIFIER ?? 'jp.nonbili.meron'
const releaseNotesPath = process.env.MAC_RELEASE_NOTES_PATH
  ?? resolve(mobileDir, 'fastlane/metadata/mac/en-US/release_notes.txt')
const skipBinaryUpload = envFlag('MAC_SKIP_BINARY_UPLOAD', false)
// Build by default, as the iOS script does. Nothing to build when the binary
// upload is being skipped.
const skipBuild = envFlag('SKIP_BUILD', skipBinaryUpload)
const submitForReview = envFlag('MAC_SUBMIT_FOR_REVIEW', true)
const rejectIfPossible = envFlag('MAC_REJECT_IF_POSSIBLE', submitForReview)
const automaticRelease = envFlag('MAC_AUTOMATIC_RELEASE', true)

// CFBundleVersion as stamped into the bundle. App Store Connect dedupes on it,
// so a re-upload of the same version needs a higher one. Tracked independently
// of the marketing version, the way CURRENT_PROJECT_VERSION is on iOS, in the
// Info.plist template that already owns the key — not in wails.json, which the
// wails CLI rewrites from a Go struct that has no field for it. It is handed to
// the build below so the package and this upload cannot disagree about it.
const infoPlistPath = resolve(repoRoot, 'desktop/build/darwin/Info.plist')
const infoPlist = await Bun.file(infoPlistPath).text()
const buildNumberMatch = infoPlist.match(
  /<key>CFBundleVersion<\/key>\s*<string>([^<]+)<\/string>/,
)
if (!buildNumberMatch || buildNumberMatch[1].includes('{{')) {
  fail(`No literal CFBundleVersion found in ${infoPlistPath}; set one or pass MAS_BUILD_NUMBER.`)
}
const buildNumber = process.env.MAS_BUILD_NUMBER ?? buildNumberMatch![1]

if (!skipBuild && pkgPath !== defaultPkgPath) {
  fail(`scripts/build-mas.sh always writes ${defaultPkgPath}; set SKIP_BUILD=1 to upload ${pkgPath} as-is.`)
}

requireEnv('APP_STORE_KEY_ID')
requireEnv('APP_STORE_ISSUER_ID')

if (!process.env.APP_STORE_KEY_FILEPATH && !process.env.APP_STORE_KEY && !process.env.APP_STORE_KEY_CONTENT) {
  fail('set APP_STORE_KEY_FILEPATH or APP_STORE_KEY.')
}
if (process.env.APP_STORE_KEY_FILEPATH) {
  await ensureFile(
    process.env.APP_STORE_KEY_FILEPATH,
    `App Store Connect API key file not found: ${process.env.APP_STORE_KEY_FILEPATH}`,
  )
}
if (!(await commandExists('bundle'))) {
  fail("bundle not found. Run 'bundle install' in mobile/.")
}

// The desktop changelog lives at the repo root, keyed by version — the same file
// the GitHub release note is built from.
const source = resolve(repoRoot, `metadata/changelogs/v${version}.txt`)
await ensureFile(source, `Changelog not found: ${source}`)
await copyFile(source, releaseNotesPath)
console.log(`Using release notes from ${source}`)

if (!skipBuild) {
  console.log(`Building ${version} (${buildNumber}) for the Mac App Store...`)
  // build-mas.sh reads its signing config (MAS_PROVISION_PROFILE,
  // APP_STORE_DEVELOPMENT_TEAM, MAS_ARCHS, ...) straight from the environment,
  // which run() passes through.
  await run([resolve(repoRoot, 'scripts/build-mas.sh')], {
    env: { MAS_BUILD_NUMBER: buildNumber },
  })
}

if (!skipBinaryUpload) {
  await ensureFile(
    pkgPath,
    skipBuild
      ? `PKG not found: ${pkgPath}. Run scripts/build-mas.sh first, or drop SKIP_BUILD to build it here.`
      : `PKG not found: ${pkgPath}.`,
  )
}

if (skipBinaryUpload && submitForReview) {
  console.log(`Submitting existing App Store Connect build ${version} (${buildNumber}) for ${appIdentifier}...`)
} else if (submitForReview) {
  console.log(`Uploading ${pkgPath} to App Store Connect and submitting for review for ${appIdentifier}...`)
} else {
  console.log(`Uploading ${pkgPath} to App Store Connect for ${appIdentifier}...`)
}

// deliver applies reject_if_possible only after it has tried to create the new
// App Store version, which App Store Connect refuses while an older version is
// still in review. So cancel first, in its own lane. Note this returns the
// pending version to "Prepare for Submission" and the upload below renames it,
// so the version still in review never ships on its own.
if (rejectIfPossible) {
  console.log('Cancelling any in-progress review submission first...')
  await run(['bundle', 'exec', 'fastlane', 'mac', 'cancel_review'], {
    cwd: mobileDir,
    env: { MAC_APP_IDENTIFIER: appIdentifier },
  })
}

await run(['bundle', 'exec', 'fastlane', 'mac', 'upload_pkg'], {
  cwd: mobileDir,
  env: {
    MAC_APP_IDENTIFIER: appIdentifier,
    MAC_APP_VERSION: version,
    MAC_BUILD_NUMBER: buildNumber,
    PKG_PATH: pkgPath,
    MAC_RELEASE_NOTES_PATH: releaseNotesPath,
    MAC_SKIP_BINARY_UPLOAD: skipBinaryUpload ? '1' : '0',
    MAC_SUBMIT_FOR_REVIEW: submitForReview ? '1' : '0',
    MAC_REJECT_IF_POSSIBLE: rejectIfPossible ? '1' : '0',
    MAC_AUTOMATIC_RELEASE: automaticRelease ? '1' : '0',
    MAC_USES_ENCRYPTION: '0',
  },
})

console.log('Done.')
