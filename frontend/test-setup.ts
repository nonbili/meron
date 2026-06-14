// Minimal browser-global stubs for pure-logic tests run under `bun test`.
// Some states/* modules touch localStorage at import time (theme bootstrap).
const storage = new Map<string, string>()
;(globalThis as any).localStorage ??= {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => void storage.set(key, String(value)),
  removeItem: (key: string) => void storage.delete(key),
  clear: () => storage.clear(),
}

// settings.ts applies the theme to <html> (class + inline CSS vars) and
// consults matchMedia on import.
const classes = new Set<string>()
const inlineStyles = new Map<string, string>()
;(globalThis as any).document ??= {
  documentElement: {
    classList: {
      toggle: (name: string, force?: boolean) =>
        void ((force ?? !classes.has(name)) ? classes.add(name) : classes.delete(name)),
      add: (name: string) => void classes.add(name),
      remove: (name: string) => void classes.delete(name),
      contains: (name: string) => classes.has(name),
    },
    style: {
      setProperty: (name: string, value: string) => void inlineStyles.set(name, value),
      removeProperty: (name: string) => void inlineStyles.delete(name),
      getPropertyValue: (name: string) => inlineStyles.get(name) ?? '',
    },
  },
}
;(globalThis as any).window ??= globalThis
;(globalThis as any).matchMedia ??= () => ({
  matches: false,
  addEventListener: () => {},
  removeEventListener: () => {},
})
