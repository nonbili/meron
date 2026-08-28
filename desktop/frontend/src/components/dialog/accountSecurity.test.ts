import { describe, expect, it } from 'bun:test'
import { securityAfterPortEdit, serverSelectionAfterDiscovery } from './accountSecurity'

describe('securityAfterPortEdit', () => {
  it('follows standard ports while the selector is untouched', () => {
    expect(securityAfterPortEdit('tls', false, '143')).toBe('starttls')
    expect(securityAfterPortEdit('tls', false, '587')).toBe('starttls')
    expect(securityAfterPortEdit('starttls', false, '993')).toBe('tls')
  })

  it('preserves explicit and incomplete values', () => {
    expect(securityAfterPortEdit('none', true, '993')).toBe('none')
    expect(securityAfterPortEdit('starttls', false, '')).toBe('starttls')
  })
})

describe('serverSelectionAfterDiscovery', () => {
  it('refreshes settings previously supplied by discovery', () => {
    expect(serverSelectionAfterDiscovery('993', 'tls', false, false, false, 143)).toEqual({
      port: '143',
      security: 'starttls',
    })
  })

  it('preserves user-owned host, port, and security choices', () => {
    expect(serverSelectionAfterDiscovery('143', 'starttls', true, true, false, 993)).toEqual({
      port: '143',
      security: 'starttls',
    })
    expect(serverSelectionAfterDiscovery('143', 'starttls', true, false, true, 993)).toEqual({
      port: '143',
      security: 'starttls',
    })
  })
})
