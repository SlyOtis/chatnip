import { describe, expect, it } from 'vitest'
import { placePopover, pointBox } from './popover'

const viewport = { width: 1200, height: 800 }
const size = { width: 240, height: 200 }

describe('popover placement', () => {
  it('opens below the trigger, right aligned', () => {
    expect(placePopover({ left: 900, top: 40, width: 32, height: 32 }, size, viewport)).toMatchObject({ left: 692, top: 78 })
  })

  it('flips above when there is no room below', () => {
    expect(placePopover({ left: 900, top: 700, width: 32, height: 32 }, size, viewport)).toMatchObject({ left: 692, top: 494 })
  })

  it('prefers above when asked and flips below when above is short', () => {
    expect(placePopover({ left: 900, top: 700, width: 32, height: 32 }, size, viewport, { side: 'above' })).toMatchObject({ top: 494 })
    expect(placePopover({ left: 900, top: 40, width: 32, height: 32 }, size, viewport, { side: 'above' })).toMatchObject({ top: 78 })
  })

  it('left aligns near the left edge and shifts into the window', () => {
    expect(placePopover({ left: 20, top: 40, width: 32, height: 32 }, size, viewport)).toMatchObject({ left: 20, top: 78 })
    expect(placePopover(pointBox(1190, 790), size, viewport)).toMatchObject({ left: 950, top: 584 })
    expect(placePopover({ left: 1100, top: 40, width: 32, height: 32 }, size, viewport, { align: 'start' })).toMatchObject({ left: 892 })
  })

  it('shrinks with a scroll when neither side fits', () => {
    const tall = { width: 240, height: 1000 }
    const spot = placePopover({ left: 600, top: 400, width: 32, height: 32 }, tall, viewport)
    expect(spot.maxHeight).toBe(784)
    expect(spot.top).toBe(8)
  })

  it('caps the width to the window', () => {
    const wide = { width: 2000, height: 100 }
    expect(placePopover({ left: 600, top: 40, width: 32, height: 32 }, wide, viewport)).toMatchObject({ left: 8, maxWidth: 1184 })
  })

  it('keeps the measured room as the height cap when it fits', () => {
    expect(placePopover({ left: 900, top: 40, width: 32, height: 32 }, size, viewport).maxHeight).toBe(714)
  })
})
