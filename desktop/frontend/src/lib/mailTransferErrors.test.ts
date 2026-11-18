import { describe, expect, it } from 'bun:test'
import { mailTransferError } from './mailTransferErrors'

describe('mail transfer errors', () => {
  it('explains the observed Dovecot size rejection and confirmed progress', () => {
    const result = mailTransferError(
      'sidecar messages.copy error: Copy progress: 1/3 confirmed; message size: 12000000 bytes: IMAP APPEND copied message: no response: code: None, info: Some("[LIMIT] Mail size is larger than the maximum size allowed by server configuration")',
      'Copy failed',
    )
    expect(result).toContain('destination server rejected a message')
    expect(result).toContain('12,000,000 bytes')
    expect(result).toContain('1 of 3 messages confirmed copied')
    expect(result).toContain('retrying may create duplicates')
    expect(result).not.toContain('sidecar')
    expect(result).not.toContain('APPEND')
  })

  it.each([
    ['[TOOBIG] message rejected', 'size limit'],
    ['[OVERQUOTA] quota exceeded', 'mailbox is full'],
    ['[TRYCREATE] missing mailbox', 'folder no longer exists'],
    ['[NONEXISTENT] missing', 'folder no longer exists'],
    ['[NOPERM] denied', 'denied access'],
    ['[AUTHENTICATIONFAILED] bad credentials', 'Reconnect'],
    ['account needs reconnect: acc', 'Reconnect'],
  ])('explains server rejection %s', (message, expected) => {
    expect(mailTransferError(new Error(message), 'Move failed')).toContain(expected)
  })

  it.each(['sidecar messages.copy timeout after 5s', 'UID MOVE: connection lost'])(
    'does not imply an interrupted write can be safely retried: %s',
    (message) => {
      const result = mailTransferError({ message }, 'Move failed')
      expect(result).toContain('Move failed:')
      expect(result).toContain('Check the destination before retrying')
    },
  )

  it('gives an unknown outcome its own hint, and confirmed copies both', () => {
    const options = { uncertainHint: 'Check this folder and Trash before retrying.' }
    expect(mailTransferError('sidecar messages.delete timeout after 30s', 'Delete failed', options)).toBe(
      'Delete failed: The operation timed out before completion could be confirmed. Check this folder and Trash before retrying.',
    )
    const copied = mailTransferError(
      'Move copied messages but could not confirm removal from the source folder: connection lost',
      'Delete failed',
      options,
    )
    expect(copied).toContain('Check the destination')
    expect(copied).toMatch(/duplicates\. Check this folder and Trash before retrying\.$/)
  })

  it('treats an engine exit with the request out as an unknown outcome', () => {
    expect(mailTransferError('sidecar messages.move error: sidecar exited', 'Move failed')).toBe(
      'Move failed: The mail engine stopped before completion could be confirmed. Check the destination before retrying; retrying may create duplicates.',
    )
  })

  it('preserves unfamiliar diagnostics and handles missing error details', () => {
    expect(mailTransferError('server policy: custom restriction', 'Copy failed')).toBe(
      'server policy: custom restriction',
    )
    expect(mailTransferError(null, 'Copy failed')).toBe('Copy failed')
  })

  it('does not invent a server limit or claim the failed message was copied', () => {
    const result = mailTransferError('Copy progress: 0/2 confirmed; message size: 123 bytes: [TOOBIG]', 'Copy failed')
    expect(result).toContain('0 of 2 messages confirmed copied')
    expect(result).not.toContain('10M')
    expect(result).not.toContain('duplicates')
  })

  it('explains a move that copied successfully but failed to remove the originals', () => {
    const result = mailTransferError(
      'Move copied messages but could not confirm removal from the source folder: UID STORE: [NOPERM] permission denied',
      'Move failed',
    )
    expect(result).toContain('denied access')
    expect(result).toContain('Messages were copied to the destination')
    expect(result).toContain('removal from the source folder could not be confirmed')
    expect(result).toContain('retrying may create duplicates')
  })
})
