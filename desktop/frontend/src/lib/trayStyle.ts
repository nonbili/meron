import { invoke } from './bridge'
import { createLatestBooleanUpdater } from './latestBooleanUpdater'

export function createTrayStyleUpdater(send = (monochrome: boolean) => invoke('tray.setStyle', { monochrome })) {
  return createLatestBooleanUpdater(send)
}

export const setTrayStyle = createTrayStyleUpdater()
