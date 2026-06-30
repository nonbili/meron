#!/usr/bin/env bun

import { resolve } from 'node:path'
import {
  changelogSource,
  commandExists,
  ensureFile,
  envFlag,
  fail,
  mobileDir,
  packageInfo,
  repoRoot,
  run,
} from './release-utils'

const [trackArg, aabArg] = Bun.argv.slice(2)
const pkg = await packageInfo()
const track = trackArg ?? process.env.GOOGLE_PLAY_TRACK ?? 'production'
const aabPath = aabArg
  ?? process.env.AAB_PATH
  ?? resolve(mobileDir, 'android/build/outputs/bundle/release/android-release.aab')
const packageName = process.env.ANDROID_PACKAGE_NAME ?? 'jp.nonbili.meron'
const jsonKey = process.env.GOOGLE_PLAY_JSON_KEY ?? process.env.SUPPLY_JSON_KEY
const jsonKeyData = process.env.GOOGLE_PLAY_JSON_KEY_DATA ?? process.env.SUPPLY_JSON_KEY_DATA
const uploadChangelogs = envFlag('GOOGLE_PLAY_UPLOAD_CHANGELOGS', true)

if (!jsonKey && !jsonKeyData) {
  fail('set GOOGLE_PLAY_JSON_KEY or SUPPLY_JSON_KEY to your Play service account JSON path (or *_DATA for JSON content).')
}
if (jsonKey) {
  await ensureFile(jsonKey, `Google Play service account JSON not found: ${jsonKey}`)
}
if (!(await commandExists('bundle'))) {
  fail("bundle not found. Run 'bundle install' in mobile/.")
}

// The Play changelog (named by versionCode) is the shared source changelog.
if (uploadChangelogs) {
  const changelog = changelogSource(pkg.versionCode)
  await ensureFile(changelog, `Changelog not found: ${changelog}`)
  console.log(`Using changelog ${changelog}`)
}

if (!envFlag('SKIP_BUILD', false)) {
  const gradleArgs = ['gradle', '-p', mobileDir, ':android:bundleRelease', '-PpackageRustCore']

  const props: Record<string, string> = {
    meronVersionCode: 'MERON_VERSION_CODE',
    meronVersionName: 'MERON_VERSION_NAME',
    meronReleaseStoreFile: 'MERON_ANDROID_RELEASE_STORE_FILE',
    meronReleaseStorePassword: 'MERON_ANDROID_RELEASE_STORE_PASSWORD',
    meronReleaseKeyAlias: 'MERON_ANDROID_RELEASE_KEY_ALIAS',
    meronReleaseKeyPassword: 'MERON_ANDROID_RELEASE_KEY_PASSWORD',
  }
  for (const [prop, env] of Object.entries(props)) {
    const value = process.env[env]
    if (value) {
      gradleArgs.push(`-P${prop}=${value}`)
    }
  }
  // Forward the shared NB_UPLOAD_* keystore (also read from ~/.gradle/gradle.properties).
  for (const name of ['NB_UPLOAD_STORE_FILE', 'NB_UPLOAD_STORE_PASSWORD', 'NB_UPLOAD_KEY_ALIAS', 'NB_UPLOAD_KEY_PASSWORD']) {
    const value = process.env[name]
    if (value) {
      gradleArgs.push(`-P${name}=${value}`)
    }
  }

  console.log('Building Android release bundle...')
  await run(gradleArgs, { cwd: repoRoot })
}

await ensureFile(aabPath, `AAB not found: ${aabPath}`)

console.log(`Uploading ${aabPath} to Google Play track '${track}' for ${packageName}...`)
await run(['bundle', 'exec', 'fastlane', 'android', 'upload_aab'], {
  cwd: mobileDir,
  env: {
    ANDROID_PACKAGE_NAME: packageName,
    AAB_PATH: aabPath,
    GOOGLE_PLAY_TRACK: track,
    GOOGLE_PLAY_UPLOAD_CHANGELOGS: uploadChangelogs ? '1' : '0',
  },
})

console.log('Done.')
