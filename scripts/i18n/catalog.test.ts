import { describe, expect, test } from 'bun:test'

import {
  androidEscape,
  androidLocaleDir,
  androidName,
  flattenObject,
  foldPlurals,
  iosLocale,
  placeholders,
  validateCatalogs,
} from './catalog'

describe('catalog normalization helpers', () => {
  test('flattens nested keys and converts i18next interpolation', () => {
    expect(
      flattenObject({
        buttons: { save: 'Save {{name}}' },
        common: { moreCount: '+{{count}} more' },
      }),
    ).toEqual({
      'buttons.save': 'Save {name}',
      'common.moreCount': '+{count} more',
    })
  })

  test('folds English plural suffixes into ICU messages', () => {
    expect(
      foldPlurals({
        'chat.fileItems_one': '{{count}} file',
        'chat.fileItems_other': '{{count}} files',
      }),
    ).toEqual({
      'chat.fileItems': '{count, plural, one {{{count}} file} other {{{count}} files}}',
    })
  })

  test('preserves richer Arabic plural categories', () => {
    expect(
      foldPlurals({
        'mail.threadCount_zero': 'لا رسائل',
        'mail.threadCount_one': 'رسالة واحدة',
        'mail.threadCount_two': 'رسالتان',
        'mail.threadCount_few': '{{count}} رسائل',
        'mail.threadCount_many': '{{count}} رسالة',
        'mail.threadCount_other': '{{count}} رسالة',
      }),
    ).toEqual({
      'mail.threadCount':
        '{count, plural, zero {لا رسائل} one {رسالة واحدة} two {رسالتان} few {{{count}} رسائل} many {{{count}} رسالة} other {{{count}} رسالة}}',
    })
  })

  test('does not fold incomplete plural groups', () => {
    expect(
      foldPlurals({
        'mail.threadCount_one': 'One thread',
      }),
    ).toEqual({
      'mail.threadCount_one': 'One thread',
    })
  })
})

describe('catalog validation helpers', () => {
  test('validates placeholder parity against English', () => {
    expect(() =>
      validateCatalogs({
        en: { 'welcome.message': 'Hello {name}' },
        ja: { 'welcome.message': 'こんにちは {name}' },
      }),
    ).not.toThrow()

    expect(() =>
      validateCatalogs({
        en: { 'welcome.message': 'Hello {name}' },
        ja: { 'welcome.message': 'こんにちは {user}' },
      }),
    ).toThrow(/placeholder mismatch/)
  })

  test('extracts named placeholders but ignores plural count', () => {
    expect(
      placeholders('{count, plural, one {{count} file from {sender}} other {{count} files from {sender}}}'),
    ).toEqual(['sender'])
  })
})

describe('platform mapping helpers', () => {
  test('maps special locale identifiers explicitly', () => {
    expect(androidLocaleDir('pt_BR')).toBe('values-pt-rBR')
    expect(androidLocaleDir('zh_Hans')).toBe('values-b+zh+Hans')
    expect(androidLocaleDir('zh_Hant')).toBe('values-b+zh+Hant')
    expect(iosLocale('pt_BR')).toBe('pt-BR')
    expect(iosLocale('zh_Hans')).toBe('zh-Hans')
    expect(iosLocale('zh_Hant')).toBe('zh-Hant')
  })

  test('escapes Android XML string values', () => {
    expect(androidName('buttons.cancel')).toBe('buttons_cancel')
    expect(androidEscape("@Don't use <x> or 50%")).toBe("\\@Don\\'t use &lt;x&gt; or 50%%")
  })
})
