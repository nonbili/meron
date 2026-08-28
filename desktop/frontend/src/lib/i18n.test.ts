import { afterEach, describe, expect, it } from 'bun:test'
import i18n, { resolveI18nLanguageFromWebLocale, t } from './i18n'

describe('i18n', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en')
  })

  it('resolves web locales to supported catalog ids', () => {
    expect(resolveI18nLanguageFromWebLocale('pt-BR')).toBe('pt_BR')
    expect(resolveI18nLanguageFromWebLocale('zh-TW')).toBe('zh_Hant')
    expect(resolveI18nLanguageFromWebLocale('zh-Hans-CN')).toBe('zh_Hans')
  })

  it('formats ICU plurals from the generated catalog', async () => {
    await i18n.changeLanguage('en')
    expect(t('chat.showImages', { count: 1 })).toBe('Show image')
    expect(t('chat.showImages', { count: 3 })).toBe('Show 3 images')

    await i18n.changeLanguage('de')
    expect(t('chat.showImages', { count: 2 })).toBe('2 Bilder anzeigen')
  })

  it('uses defaultValue for compatibility when a key is missing', () => {
    expect(t('missing.key', { defaultValue: 'Fallback' })).toBe('Fallback')
  })
})
