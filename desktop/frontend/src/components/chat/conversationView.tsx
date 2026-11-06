import { createContext, useContext, useLayoutEffect, useMemo } from 'react'
import { observable } from '@legendapp/state'
import { sessionRemoteReveals, thread$ } from '../../states/thread'

// Commands outside the pane still address thread$. Only the active view bridges
// its persistent state to that interface; hidden views keep their own copy.
const viewFields = [
  'search',
  'searchOpen',
  'searchFocus',
  'activeSearchIndex',
  'activeSearchId',
  'activeSearchOffset',
  'searchMatchCount',
  'expandAll',
  'expandAllAction',
  'flashMessageId',
  'revealedRemote',
] as const

export function createConversationState() {
  const local = observable({ ...thread$.peek() })
  local.assign({
    search: '',
    searchOpen: false,
    activeSearchIndex: 0,
    activeSearchId: '',
    activeSearchOffset: -1,
    searchMatchCount: 0,
    expandAll: null,
    expandAllAction: null,
    flashMessageId: '',
    revealedRemote: sessionRemoteReveals(),
  })
  // Account render preferences stay shared deliberately. Pending jumps and
  // transient menus stay global and are handled only by the active view.
  const state = new Proxy(local, {
    get(target, key, receiver) {
      if (viewFields.includes(key as (typeof viewFields)[number])) return Reflect.get(target, key, receiver)
      return Reflect.get(thread$, key, thread$)
    },
  })
  return { local, state }
}

export function bridgeConversationState(local: typeof thread$) {
  let copying = false
  const copy = (from: typeof thread$, to: typeof thread$) => {
    if (copying) return
    copying = true
    try {
      for (const field of viewFields) {
        const value = from[field].peek()
        const previous = to[field].peek()
        if (value === previous) continue
        if (value && previous && typeof value === 'object' && typeof previous === 'object') {
          const entries = Object.entries(value)
          if (
            entries.length === Object.keys(previous).length &&
            entries.every(([key, item]) => (previous as Record<string, unknown>)[key] === item)
          )
            continue
        }
        // Independent observables must not share a mutable map: a nested write
        // in one would change the other's data without notifying its readers.
        ;(to[field] as { set(value: unknown): void }).set(value && typeof value === 'object' ? { ...value } : value)
      }
    } finally {
      copying = false
    }
  }
  copy(local, thread$)
  const offGlobal = thread$.onChange(() => copy(thread$, local))
  const offLocal = local.onChange(() => copy(local, thread$))
  return () => {
    offGlobal()
    offLocal()
  }
}

export const ConversationViewContext = createContext({ state: thread$, active: true, scope: '' })
export function useConversationView() {
  return useContext(ConversationViewContext)
}
export function useConversationState(active: boolean) {
  const view = useMemo(createConversationState, [])
  useLayoutEffect(() => (active ? bridgeConversationState(view.local) : undefined), [active, view])
  return view.state
}
