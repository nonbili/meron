import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import arText from '../locales/ar.json'
import deText from '../locales/de.json'
import elText from '../locales/el.json'
import enText from '../locales/en.json'
import esText from '../locales/es.json'
import etText from '../locales/et.json'
import frText from '../locales/fr.json'
import itText from '../locales/it.json'
import koText from '../locales/ko.json'
import lvText from '../locales/lv.json'
import plText from '../locales/pl.json'
import ptText from '../locales/pt.json'
import ptBRText from '../locales/pt_BR.json'
import svText from '../locales/sv.json'
import trText from '../locales/tr.json'
import viText from '../locales/vi.json'
import zhHansText from '../locales/zh_Hans.json'
import zhHantText from '../locales/zh_Hant.json'
import jaText from '../locales/ja.json'

export const supportedI18nLanguages = [
  'ar',
  'de',
  'el',
  'en',
  'es',
  'et',
  'fr',
  'it',
  'ko',
  'lv',
  'pl',
  'pt',
  'pt_BR',
  'sv',
  'tr',
  'vi',
  'zh_Hans',
  'zh_Hant',
  'ja',
] as const

export type SupportedI18nLanguage = (typeof supportedI18nLanguages)[number]

export const languageNativeNames: Record<SupportedI18nLanguage, string> = {
  ar: 'العربية',
  de: 'Deutsch',
  el: 'Ελληνικά',
  en: 'English',
  es: 'Español',
  et: 'Eesti',
  fr: 'Français',
  it: 'Italiano',
  ko: '한국어',
  lv: 'Latviešu',
  pl: 'Polski',
  pt: 'Português',
  pt_BR: 'Português (Brasil)',
  sv: 'Svenska',
  tr: 'Türkçe',
  vi: 'Tiếng Việt',
  zh_Hans: '简体中文',
  zh_Hant: '繁體中文',
  ja: '日本語',
}

const resources: Record<SupportedI18nLanguage, { translation: any }> = {
  ar: { translation: arText },
  de: { translation: deText },
  el: { translation: elText },
  en: { translation: enText },
  es: { translation: esText },
  et: { translation: etText },
  fr: { translation: frText },
  it: { translation: itText },
  ko: { translation: koText },
  lv: { translation: lvText },
  pl: { translation: plText },
  pt: { translation: ptText },
  pt_BR: { translation: ptBRText },
  sv: { translation: svText },
  tr: { translation: trText },
  vi: { translation: viText },
  zh_Hans: { translation: zhHansText },
  zh_Hant: { translation: zhHantText },
  ja: { translation: jaText },
}

const isSupportedLanguage = (value?: string | null): value is SupportedI18nLanguage =>
  Boolean(value && supportedI18nLanguages.includes(value as any))

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

i18n.use(initReactI18next).init({
  fallbackLng: 'en',
  supportedLngs: Object.keys(resources),
  resources,
  interpolation: {
    escapeValue: false, // react already escapes values to prevent XSS
  },
})

export default i18n
