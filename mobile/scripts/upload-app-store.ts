#!/usr/bin/env bun

import { resolve } from 'node:path'
import {
  changelogSource,
  commandExists,
  copyFile,
  ensureFile,
  envFlag,
  fail,
  mobileDir,
  packageInfo,
  requireEnv,
  run,
} from './release-utils'

const [ipaArg] = Bun.argv.slice(2)
const pkg = await packageInfo()
const ipaPath = ipaArg ?? process.env.IPA_PATH ?? resolve(mobileDir, 'ios/build/Meron.ipa')
const appIdentifier = process.env.IOS_APP_IDENTIFIER ?? 'jp.nonbili.meron'
const releaseNotesPath = process.env.IOS_RELEASE_NOTES_PATH
  ?? resolve(mobileDir, 'fastlane/metadata/ios/en-US/release_notes.txt')
const skipBinaryUpload = envFlag('IOS_SKIP_BINARY_UPLOAD', false)
const skipBuild = envFlag('SKIP_BUILD', skipBinaryUpload)
const submitForReview = envFlag('IOS_SUBMIT_FOR_REVIEW', true)
const rejectIfPossible = envFlag('IOS_REJECT_IF_POSSIBLE', submitForReview)
const automaticRelease = envFlag('IOS_AUTOMATIC_RELEASE', true)

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

// Derive the iOS release notes from the shared source changelog (keyed by versionCode).
const source = changelogSource(pkg.versionCode)
await ensureFile(source, `Changelog not found: ${source}`)
await copyFile(source, releaseNotesPath)
console.log(`Using release notes from ${source}`)

if (!skipBinaryUpload && skipBuild) {
  await ensureFile(ipaPath, `IPA not found: ${ipaPath}`)
}

if (skipBinaryUpload && submitForReview) {
  console.log(`Submitting existing App Store Connect build ${pkg.version} (${pkg.buildNumber}) for ${appIdentifier}...`)
} else if (submitForReview) {
  console.log(`Uploading ${ipaPath} to App Store Connect and submitting for review for ${appIdentifier}...`)
} else {
  console.log(`Uploading ${ipaPath} to App Store Connect for ${appIdentifier}...`)
}

await run(['bundle', 'exec', 'fastlane', 'ios', 'upload_ipa'], {
  cwd: mobileDir,
  env: {
    IOS_APP_IDENTIFIER: appIdentifier,
    IOS_APP_VERSION: pkg.version,
    IOS_BUILD_NUMBER: pkg.buildNumber,
    IPA_PATH: ipaPath,
    IOS_RELEASE_NOTES_PATH: releaseNotesPath,
    IOS_BUILD_BEFORE_UPLOAD: skipBuild ? '0' : '1',
    IOS_SKIP_BINARY_UPLOAD: skipBinaryUpload ? '1' : '0',
    IOS_SUBMIT_FOR_REVIEW: submitForReview ? '1' : '0',
    IOS_REJECT_IF_POSSIBLE: rejectIfPossible ? '1' : '0',
    IOS_AUTOMATIC_RELEASE: automaticRelease ? '1' : '0',
  },
})

console.log('Done.')
