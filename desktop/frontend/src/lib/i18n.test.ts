import { afterEach, describe, expect, it } from 'bun:test'
import { messages } from '../generated/locales'
import i18n, { normalizeI18nLanguage, resolveI18nLanguageFromWebLocale, t } from './i18n'

describe('i18n', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en')
  })

  it('resolves web locales to supported catalog ids', () => {
    expect(resolveI18nLanguageFromWebLocale('ru-RU')).toBe('ru')
    expect(resolveI18nLanguageFromWebLocale('ru_BY')).toBe('ru')
    expect(resolveI18nLanguageFromWebLocale('RU')).toBe('ru')
    expect(normalizeI18nLanguage('ru')).toBe('ru')
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

  it('switches to Russian and formats all Russian plural categories', async () => {
    await i18n.changeLanguage('ru')
    expect(i18n.language).toBe('ru')
    expect(t('buttons.cancel')).toBe('Отмена')
    expect(t('about.version', { version: '1.2.3' })).toBe('Версия 1.2.3')
    for (const [count, ending] of [
      [0, 'писем'],
      [1, 'письмо'],
      [2, 'письма'],
      [4, 'письма'],
      [5, 'писем'],
      [11, 'писем'],
      [12, 'писем'],
      [14, 'писем'],
      [21, 'письмо'],
      [22, 'письма'],
      [25, 'писем'],
      [101, 'письмо'],
      [111, 'писем'],
      [1.5, 'письма'],
    ] as const) {
      expect(t('chat.messagesCount', { count })).toBe(`${count} ${ending}`)
    }
    expect(t('mobile.refresh.failed', { failed: 22 })).toBe('Не удалось обновить 22 аккаунта')
    expect(t('mobile.refresh.refreshedWithFailures', { refreshed: 21, failed: 2 })).toBe(
      'Обновлён 21 аккаунт, с ошибками: 2',
    )
    await i18n.changeLanguage('en')
    expect(t('buttons.cancel')).toBe('Cancel')
  })

  it('formats every Russian message with its required arguments', async () => {
    await i18n.changeLanguage('ru')
    for (const [key, message] of Object.entries(messages.ru)) {
      const values: Record<string, string | number> = {}
      for (const match of message.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)(?:\s*,\s*(number|plural))?\s*[,}]/g)) {
        values[match[1]] = match[2] ? 22 : 'пример'
      }
      // Inner {count} occurrences must retain the numeric plural argument.
      for (const match of message.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\s*,\s*(?:number|plural)/g)) {
        values[match[1]] = 22
      }
      expect(() => t(key, values)).not.toThrow()
      expect(t(key, values)).not.toContain('{')
    }
  })

  it('uses defaultValue for compatibility when a key is missing', () => {
    expect(t('missing.key', { defaultValue: 'Fallback' })).toBe('Fallback')
  })
})
