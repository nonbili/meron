import {
  Fragment,
  cloneElement,
  createElement,
  isValidElement,
  useEffect,
  useMemo,
  useState,
  type ReactElement,
  type ReactNode,
} from 'react'
import IntlMessageFormat from 'intl-messageformat'
import { languageNativeNames, messages, supportedI18nLanguages, type SupportedI18nLanguage } from '../generated/locales'

export { languageNativeNames, supportedI18nLanguages, type SupportedI18nLanguage }

type TranslationValues = Record<string, unknown> & { defaultValue?: string }
type Listener = () => void

const listeners = new Set<Listener>()
const formatterCache = new Map<string, IntlMessageFormat>()

const localeForIntl = (language: SupportedI18nLanguage) => language.replace('_', '-')

const isSupportedLanguage = (value?: string | null): value is SupportedI18nLanguage =>
  Boolean(value && supportedI18nLanguages.includes(value as any))

let currentLanguage: SupportedI18nLanguage = 'en'

function notify() {
  for (const listener of listeners) listener()
}

function messageFor(language: SupportedI18nLanguage, key: string, defaultValue?: string) {
  return messages[language]?.[key] ?? messages.en[key] ?? defaultValue ?? key
}

function formatMessage(language: SupportedI18nLanguage, key: string, values: TranslationValues = {}) {
  const source = messageFor(language, key, values.defaultValue)
  const cacheKey = `${language}\0${key}\0${source}`
  let formatter = formatterCache.get(cacheKey)
  if (!formatter) {
    formatter = new IntlMessageFormat(source, localeForIntl(language), undefined, { ignoreTag: false })
    formatterCache.set(cacheKey, formatter)
  }
  return formatter.format(values)
}

export const resolveI18nLanguageFromWebLocale = (localeStr?: string): SupportedI18nLanguage | undefined => {
  if (!localeStr) {
    return undefined
  }
  const normalized = localeStr.replace('_', '-')
  const parts = normalized.split('-')
  const lang = parts[0].toLowerCase()

  if (lang === 'zh') {
    if (parts.includes('Hans')) return 'zh_Hans'
    if (parts.includes('Hant')) return 'zh_Hant'
    const region = parts[1]?.toUpperCase()
    return region === 'TW' || region === 'HK' || region === 'MO' || region === 'CHT' ? 'zh_Hant' : 'zh_Hans'
  }

  if (lang === 'pt') {
    const region = parts[1]?.toUpperCase()
    return region === 'BR' ? 'pt_BR' : 'pt'
  }

  return isSupportedLanguage(lang) ? (lang as SupportedI18nLanguage) : undefined
}

export const normalizeI18nLanguage = (value?: string | null): SupportedI18nLanguage | null =>
  value == null ? null : isSupportedLanguage(value) ? value : null

export function t(key: string, values: TranslationValues = {}): string {
  const formatted = formatMessage(currentLanguage, key, values)
  return Array.isArray(formatted) ? formatted.join('') : String(formatted)
}

export function useTranslation() {
  const [language, setLanguage] = useState(currentLanguage)

  useEffect(() => {
    const listener = () => setLanguage(currentLanguage)
    listeners.add(listener)
    return () => void listeners.delete(listener)
  }, [])

  return useMemo(
    () => ({
      t: (key: string, values: TranslationValues = {}) => {
        const formatted = formatMessage(language, key, values)
        return Array.isArray(formatted) ? formatted.join('') : String(formatted)
      },
      i18n,
    }),
    [language],
  )
}

export function Trans({
  i18nKey,
  values = {},
  components = {},
}: {
  i18nKey: string
  values?: TranslationValues
  components?: Record<string, ReactElement>
}) {
  const [language, setLanguage] = useState(currentLanguage)

  useEffect(() => {
    const listener = () => setLanguage(currentLanguage)
    listeners.add(listener)
    return () => void listeners.delete(listener)
  }, [])

  const richValues = useMemo(() => {
    const out: Record<string, unknown> = { ...values }
    for (const [name, component] of Object.entries(components)) {
      out[name] = (chunks: ReactNode[]) =>
        isValidElement(component) ? cloneElement(component, { key: name }, chunks) : chunks
    }
    return out
  }, [components, values])

  const formatted = formatMessage(language, i18nKey, richValues)
  return createElement(Fragment, null, formatted as ReactNode)
}

const i18n = {
  get language() {
    return currentLanguage
  },
  async changeLanguage(language: string) {
    const next = normalizeI18nLanguage(language) ?? 'en'
    if (next !== currentLanguage) {
      currentLanguage = next
      notify()
    }
    return next
  },
}

export default i18n
