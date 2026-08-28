// Browser environment for `bun test`.
//
// Most of the suite is pure logic, but the parts worth testing hardest — the
// composer's autosave, the settings editors' ownership of what they save — are
// React wiring, and wiring only misbehaves once state, effects and unmounts are
// real. happy-dom gives those tests a DOM to render into; the pure-logic tests
// neither notice nor pay for it (registration is a few milliseconds).
import { GlobalRegistrator } from '@happy-dom/global-registrator'

if (!(globalThis as any).document) {
  GlobalRegistrator.register({ url: 'https://meron.test/' })
}

// React 19's `act` needs this to keep effect flushing deterministic; without it
// every render logs a warning about updates not wrapped in act().
;(globalThis as any).IS_REACT_ACT_ENVIRONMENT = true

// happy-dom implements matchMedia, but not the query parsing settings.ts relies
// on at import time; a permissive stub keeps the theme bootstrap deterministic.
;(globalThis as any).matchMedia ??= () => ({
  matches: false,
  addEventListener: () => {},
  removeEventListener: () => {},
})
