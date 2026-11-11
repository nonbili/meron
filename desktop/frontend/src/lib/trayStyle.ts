import { invoke } from './bridge'
import { createLatestValueUpdater } from './latestValueUpdater'

export function createTrayStyleUpdater(send = (monochrome: boolean) => invoke('tray.setStyle', { monochrome })) {
  return createLatestValueUpdater(send)
}

export const setTrayStyle = createTrayStyleUpdater()
