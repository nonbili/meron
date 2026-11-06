import { invoke } from './bridge'

// All entry points in a window share the pending picker, including menu clicks
// made while a keyboard shortcut's native dialog is still opening.
let pendingPicker: Promise<unknown> | undefined

export function openEmlFile(session?: string): Promise<unknown> {
  if (!pendingPicker) {
    pendingPicker = invoke('eml.open', session ? { session } : {}).finally(() => {
      pendingPicker = undefined
    })
  }
  return pendingPicker
}
