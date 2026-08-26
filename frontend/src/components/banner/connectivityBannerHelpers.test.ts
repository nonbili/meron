import { describe, expect, it } from 'bun:test'
import {
  connectivityAccountLabel,
  isConnectivitySyncError,
  proxyEndpointFromSyncError,
} from './connectivityBannerHelpers'

const accounts = [
  { id: 'one', display_name: 'Ping Chen', email: 'ping.one@example.com' },
  { id: 'two', display_name: 'Ping Chen', email: 'ping.two@example.com' },
  { id: 'three', display_name: 'Ada', email: 'ada@example.com' },
]

describe('connectivityAccountLabel', () => {
  it('adds the email address when display names collide', () => {
    expect(connectivityAccountLabel('one', accounts)).toBe('Ping Chen (ping.one@example.com)')
    expect(connectivityAccountLabel('two', accounts)).toBe('Ping Chen (ping.two@example.com)')
  })

  it('keeps a unique display name concise and falls back safely', () => {
    expect(connectivityAccountLabel('three', accounts)).toBe('Ada')
    expect(connectivityAccountLabel('missing', accounts)).toBe('missing')
    expect(connectivityAccountLabel(null, accounts)).toBeNull()
  })
})

describe('proxyEndpointFromSyncError', () => {
  it('extracts proxy endpoints from connection and handshake errors', () => {
    expect(
      proxyEndpointFromSyncError(
        'sync inbox: connect to proxy 127.0.0.1:1: tcp connect: connect 127.0.0.1:1: Connection refused',
      ),
    ).toBe('127.0.0.1:1')
    expect(
      proxyEndpointFromSyncError('sync inbox: socks5 proxy proxy.example:1080 to imap.example:993: timed out'),
    ).toBe('proxy.example:1080')
    expect(proxyEndpointFromSyncError('sync inbox: connect to proxy [::1]:9050: Connection refused')).toBe('[::1]:9050')
  })

  it('does not misclassify a mail-server failure', () => {
    expect(proxyEndpointFromSyncError('sync inbox: connect imap.example:993: timed out')).toBeNull()
  })
})

describe('isConnectivitySyncError', () => {
  it('ignores a marked outer background-sync budget expiry', () => {
    expect(isConnectivitySyncError(true)).toBe(false)
  })

  it('keeps every unmarked failure visible', () => {
    expect(isConnectivitySyncError()).toBe(true)
    expect(isConnectivitySyncError(false)).toBe(true)
  })
})
