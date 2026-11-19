// Sign the debug meron-core with a stable identity, so macOS remembers
// "Always Allow" for its keychain items across rebuilds.
//
// The linker signs every build ad hoc, and an ad-hoc signature is identified by
// the binary's hash: to the keychain each rebuild is a new app, which has to be
// let into every item (the store key, each account's password) again. Signed
// with a certificate and a fixed identifier, it stays the same app.
//
// The identity is MERON_CODESIGN_IDENTITY when set, else a "Meron Dev"
// certificate, else an "Apple Development" one, else any code-signing identity
// in the keychain. Without one this does nothing: the build works as before,
// prompts included. CONTRIBUTING.md says how to make a self-signed one.
//
// Takes about a tenth of a second. macOS only; elsewhere it exits at once.

import { existsSync } from 'node:fs'
import { resolve } from 'node:path'

const IDENTIFIER = 'jp.nonbili.meron.core-dev'
const PREFERRED = ['Meron Dev', 'Apple Development']

if (process.platform !== 'darwin') process.exit(0)

const binary = resolve(import.meta.dir, '../meron-core/target/debug/meron-core')
if (!existsSync(binary)) process.exit(0)

function pickIdentity(): string | null {
  const chosen = process.env.MERON_CODESIGN_IDENTITY?.trim()
  if (chosen) return chosen
  const listed = Bun.spawnSync(['security', 'find-identity', '-v', '-p', 'codesigning'])
  // `  1) <SHA-1> "Apple Development: …"`: sign by hash, which names exactly one.
  const identities = listed.stdout
    .toString()
    .split('\n')
    .map((line) => line.match(/^\s*\d+\)\s+([0-9A-F]{40})\s+"(.*)"/))
    .filter((match): match is RegExpMatchArray => match !== null)
    .map(([, hash, name]) => ({ hash, name }))
  for (const prefix of PREFERRED) {
    const found = identities.find((identity) => identity.name.startsWith(prefix))
    if (found) return found.hash
  }
  return identities[0]?.hash ?? null
}

const identity = pickIdentity()
if (!identity) {
  console.warn(
    'sign-dev-core: no code-signing identity, so macOS will ask for keychain access after each rebuild (see CONTRIBUTING.md)',
  )
  process.exit(0)
}

const signed = Bun.spawnSync(['codesign', '--force', '--sign', identity, '--identifier', IDENTIFIER, binary])
if (signed.exitCode !== 0) {
  // Never fail the build over this: an unsigned sidecar still runs.
  console.warn(`sign-dev-core: codesign failed, keychain prompts will continue:\n${signed.stderr.toString().trim()}`)
}
