#!/usr/bin/env bun

import { dirname, resolve } from 'node:path'

type PackageInfo = {
  version: string
  versionCode: string
  buildNumber: string
}

// mobile/ holds the fastlane setup (incl. the shared changelogs); the repo root holds wails.json.
export const mobileDir = resolve(import.meta.dir, '..')
export const repoRoot = resolve(mobileDir, '..')

export const envFlag = (name: string, defaultValue: boolean) => {
  const value = process.env[name]
  if (value == null || value === '') {
    return defaultValue
  }

  return value !== '0'
}

export const requireEnv = (name: string) => {
  const value = process.env[name]
  if (!value) {
    fail(`${name} is required.`)
  }

  return value
}

// The Android versionCode fallback lives in build.gradle; reuse it so the
// upload script and the build agree when MERON_VERSION_CODE isn't set.
const gradleVersionCode = async (): Promise<string> => {
  const gradle = await Bun.file(resolve(mobileDir, 'android/build.gradle')).text()
  const match = gradle.match(/releaseProp\("meronVersionCode",\s*"MERON_VERSION_CODE",\s*"(\d+)"\)/)
  if (!match) {
    fail('Could not read meronVersionCode fallback from android/build.gradle')
  }

  return match[1]
}

// The iOS marketing version and build number live in Version.xcconfig
// (MARKETING_VERSION / CURRENT_PROJECT_VERSION).
const iosVersion = async (): Promise<{ marketingVersion: string, buildNumber: string }> => {
  const xcconfig = await Bun.file(resolve(mobileDir, 'ios/Version.xcconfig')).text()
  const marketing = xcconfig.match(/MARKETING_VERSION\s*=\s*(\S+)/)
  const build = xcconfig.match(/CURRENT_PROJECT_VERSION\s*=\s*(\d+)/)
  if (!marketing) {
    fail('Could not read MARKETING_VERSION from ios/Version.xcconfig')
  }
  if (!build) {
    fail('Could not read CURRENT_PROJECT_VERSION from ios/Version.xcconfig')
  }

  return { marketingVersion: marketing[1], buildNumber: build[1] }
}

// The Android versionCode and the iOS marketing version / build number each have
// their own source of truth (build.gradle / Version.xcconfig), overridable via
// the environment. The marketing version drives the iOS App Store upload.
export const packageInfo = async (): Promise<PackageInfo> => {
  const ios = await iosVersion()
  const versionCode = process.env.MERON_VERSION_CODE ?? await gradleVersionCode()
  return {
    version: ios.marketingVersion,
    versionCode,
    buildNumber: process.env.IOS_BUILD_NUMBER ?? ios.buildNumber,
  }
}

// The single source changelog, shared by both mobile stores and F-Droid:
// mobile/fastlane/metadata/android/en-US/changelogs/<versionCode>.txt
export const changelogSource = (versionCode: string) => (
  resolve(mobileDir, `fastlane/metadata/android/en-US/changelogs/${versionCode}.txt`)
)

export const fileExists = async (path: string) => (
  await Bun.file(path).exists()
)

export const ensureFile = async (path: string, message: string) => {
  if (!(await fileExists(path))) {
    fail(message)
  }
}

export const copyFile = async (source: string, destination: string) => {
  await Bun.$`mkdir -p ${dirname(destination)}`
  await Bun.write(destination, Bun.file(source))
}

export const fail = (message: string): never => {
  console.error(`Error: ${message}`)
  process.exit(1)
}

export const run = async (
  command: string[],
  options: {
    cwd?: string
    env?: Record<string, string | undefined>
  } = {},
) => {
  const subprocess = Bun.spawn(command, {
    cwd: options.cwd ?? repoRoot,
    env: {
      ...process.env,
      ...options.env,
    },
    stdout: 'inherit',
    stderr: 'inherit',
    stdin: 'inherit',
  })

  const exitCode = await subprocess.exited
  if (exitCode !== 0) {
    fail(`Command failed with exit code ${exitCode}: ${command.join(' ')}`)
  }
}

export const commandExists = async (command: string) => {
  const subprocess = Bun.spawn(['/bin/sh', '-lc', `command -v ${command}`], {
    stdout: 'ignore',
    stderr: 'ignore',
  })

  return (await subprocess.exited) === 0
}
