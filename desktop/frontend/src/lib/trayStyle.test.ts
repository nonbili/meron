import { expect, test } from 'bun:test'
import { createTrayStyleUpdater } from './trayStyle'

const tick = () => new Promise((resolve) => setTimeout(resolve, 0))

test('quick style toggles finish on the newest saved choice', async () => {
  let resolve!: () => void
  const first = new Promise<void>((done) => {
    resolve = done
  })
  const sent: boolean[] = []
  const update = createTrayStyleUpdater(async (monochrome) => {
    sent.push(monochrome)
    if (sent.length === 1) await first
  })
  update(true)
  update(false)
  update(true)
  update(false)
  expect(sent).toEqual([true])
  resolve()
  await tick()
  expect(sent).toEqual([true, false])
  update(false)
  await tick()
  expect(sent).toEqual([true, false])
})
