#!/usr/bin/env bun

import { commandExists, fail, mobileDir, run } from './release-utils'

const platform = Bun.argv[2]
if (platform !== 'ios' && platform !== 'android') {
  fail('Usage: upload-screenshots.ts ios|android')
}
if (!(await commandExists('bundle'))) {
  fail("bundle not found. Run 'bundle install' in mobile/.")
}

if (platform === 'android') {
  const jsonKey = process.env.GOOGLE_PLAY_JSON_KEY ?? process.env.SUPPLY_JSON_KEY
  if (!jsonKey && !process.env.GOOGLE_PLAY_JSON_KEY_DATA && !process.env.SUPPLY_JSON_KEY_DATA) {
    fail('set GOOGLE_PLAY_JSON_KEY to your Play service account JSON path.')
  }
  await run(['bundle', 'exec', 'fastlane', 'android', 'upload_screenshots'], {
    cwd: mobileDir,
    env: { GOOGLE_PLAY_JSON_KEY: jsonKey },
  })
} else {
  await run(['bundle', 'exec', 'fastlane', 'ios', 'upload_screenshots'], { cwd: mobileDir })
}

console.log('Done.')
