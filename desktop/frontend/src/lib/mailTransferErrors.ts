import { errorMessage } from './errors'
import { t } from './i18n'

// Backend failures can cross Wails as strings, Errors, or message objects.
// Keep the original diagnostic in the console; toasts explain what to do next.
// `uncertainHint` replaces the destination check for an unknown outcome, for
// changes like a delete that may have no destination; empty leaves it out.
// After confirmed copies it follows the duplicates warning, to say where.
// `permissionReason` words a refusal for a change without a destination. Copies the backend
// reports having made still get the duplicates warning.
export function mailTransferError(
  error: unknown,
  fallback: string,
  options: { uncertainHint?: string; permissionReason?: string } = {},
): string {
  const detail = errorMessage(error, fallback)
  console.error('Mail transfer failed:', detail)
  const lower = detail.toLowerCase()
  let reason = ''
  let uncertain = false
  if (
    /\btoobig\b|mail size is larger|message (?:size .*exceed|too (?:large|big))|maximum (?:message|mail) size/.test(
      lower,
    )
  ) {
    reason = t('mail.transferError.sizeLimit')
  } else if (/\boverquota\b|quota exceeded|mailbox (?:is )?full|insufficient (?:disk )?space/.test(lower)) {
    reason = t('mail.transferError.quota')
  } else if (/\btrycreate\b|\bnonexistent\b|mailbox does not exist|no such (?:mailbox|folder)/.test(lower)) {
    reason = t('mail.transferError.missingFolder')
  } else if (/\bnoperm\b|permission denied|not permitted|access denied/.test(lower)) {
    reason = options.permissionReason ?? t('mail.transferError.permission')
  } else if (/\bauthenticationfailed\b|authentication failed|login failed|account needs reconnect/.test(lower)) {
    reason = t('mail.transferError.authentication')
  } else if (/\bsidecar exited\b/.test(lower)) {
    // The engine quit with the request out: it may have finished it first.
    reason = t('mail.transferError.engineStopped')
    uncertain = true
  } else if (/timed? out|timeout/.test(lower)) {
    reason = t('mail.transferError.timeout')
    uncertain = true
  } else if (
    /connection (?:lost|closed|reset|refused|aborted)|broken pipe|unexpected eof|peer closed|dns lookup|failed to lookup address|network (?:is )?unreachable/.test(
      lower,
    )
  ) {
    reason = t('mail.transferError.connection')
    uncertain = true
  }

  const progress = /Copy progress: (\d+)\/(\d+) confirmed; message size: (\d+) bytes/.exec(detail)
  const copiedNotRemoved = lower.includes('move copied messages but could not confirm removal from the source folder')
  const parts = [reason ? `${fallback}: ${reason}` : copiedNotRemoved ? fallback : detail]
  if (copiedNotRemoved) parts.push(t('mail.transferError.copiedNotRemoved'))
  if (progress) {
    if (reason === t('mail.transferError.sizeLimit')) {
      parts.push(t('mail.transferError.messageSize', { bytes: Number(progress[3]) }))
    }
    parts.push(t('mail.transferError.progress', { copied: Number(progress[1]), total: Number(progress[2]) }))
  }
  if (copiedNotRemoved || (progress && Number(progress[1]) > 0)) {
    parts.push(t('mail.transferError.checkDestination'))
    // Where to look, when the caller knows better than "the destination".
    if (options.uncertainHint) parts.push(options.uncertainHint)
  } else if (uncertain) {
    const hint = options.uncertainHint ?? t('mail.transferError.checkDestination')
    if (hint) parts.push(hint)
  }
  return parts.join(' ')
}
