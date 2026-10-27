import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { getLaunchDuration, getLaunchReady, getLaunchText, getReducedMotion, LAUNCH_MS, LAUNCH_TIMEOUT_MS, launchSettled, launchStep, LOGO_MS, putLaunchFrame, SETTLE_MS, putLaunchStep, SAVE_MS, watchLaunch, watchWindowFrame, isSurface, watchWindowDrag, type FrameSource, type LaunchStep } from '$lib/window'
import { get } from 'svelte/store'
import { en } from '$lib/i18n/en'
import { nb } from '$lib/i18n/nb'

function frameSource() {
  const handlers: Record<'moved' | 'resized', (() => void)[]> = { moved: [], resized: [] }
  const stops = { moved: vi.fn(), resized: vi.fn(), focus: vi.fn() }
  const focus: ((event: { payload: boolean }) => void)[] = []
  const source = {
    onMoved: vi.fn(async (handler: () => void) => {
      handlers.moved.push(handler)
      return stops.moved
    }),
    onResized: vi.fn(async (handler: () => void) => {
      handlers.resized.push(handler)
      return stops.resized
    }),
    onFocusChanged: vi.fn(async (handler: (event: { payload: boolean }) => void) => {
      focus.push(handler)
      return stops.focus
    })
  }
  return { source: source as unknown as FrameSource, handlers, stops, focus }
}

describe('watchWindowFrame', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    launchSettled.set(true)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('saves once after moving and resizing settle', async () => {
    const { source, handlers } = frameSource()
    const save = vi.fn()
    await watchWindowFrame(source, save)

    handlers.moved[0]()
    vi.advanceTimersByTime(SAVE_MS - 100)
    handlers.resized[0]()
    vi.advanceTimersByTime(SAVE_MS - 100)
    handlers.moved[0]()
    expect(save).not.toHaveBeenCalled()

    vi.advanceTimersByTime(SAVE_MS)
    expect(save).toHaveBeenCalledTimes(1)
  })

  it('saves again for a later move', async () => {
    const { source, handlers } = frameSource()
    const save = vi.fn()
    await watchWindowFrame(source, save, 200)

    handlers.moved[0]()
    vi.advanceTimersByTime(200)
    handlers.resized[0]()
    vi.advanceTimersByTime(200)
    expect(save).toHaveBeenCalledTimes(2)
  })

  it('stops listening when unwatched', async () => {
    const { source, stops } = frameSource()
    const stop = await watchWindowFrame(source, vi.fn())
    stop()
    expect(stops.moved).toHaveBeenCalledTimes(1)
    expect(stops.resized).toHaveBeenCalledTimes(1)
  })
})

describe('launch motion', () => {
  it('animates by default and jumps under reduced motion', () => {
    expect(getLaunchDuration(false)).toBe(LAUNCH_MS)
    expect(getLaunchDuration(true)).toBe(0)
  })

  it('reads the reduced motion preference', () => {
    const query = (matches: boolean) => ({ matchMedia: vi.fn(() => ({ matches }) as MediaQueryList) })
    const reduced = query(true)
    expect(getReducedMotion(reduced)).toBe(true)
    expect(reduced.matchMedia).toHaveBeenCalledWith('(prefers-reduced-motion: reduce)')
    expect(getReducedMotion(query(false))).toBe(false)
  })
})

describe('launch gate', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    launchStep.set('starting')
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('grows once the first page is ready, not before', () => {
    const grow = vi.fn()
    watchLaunch(grow)
    vi.advanceTimersByTime(LOGO_MS)
    for (const step of ['unlocking', 'opening', 'loading'] as const) putLaunchStep(step)
    expect(grow).not.toHaveBeenCalled()
    putLaunchStep('ready')
    putLaunchStep('loading')
    expect(grow).toHaveBeenCalledTimes(1)
    expect(get(launchStep)).toBe('ready')
  })

  it('grows to show the lock screen once it accepts input', () => {
    const grow = vi.fn()
    watchLaunch(grow)
    vi.advanceTimersByTime(LOGO_MS)
    putLaunchStep('unlocking')
    putLaunchStep('locked')
    expect(grow).toHaveBeenCalledTimes(1)
  })

  it('grows anyway after the safety timeout and keeps the progress text', () => {
    const grow = vi.fn()
    watchLaunch(grow)
    putLaunchStep('loading')
    vi.advanceTimersByTime(LAUNCH_TIMEOUT_MS - 1)
    expect(grow).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(grow).toHaveBeenCalledTimes(1)
    expect(getLaunchReady(get(launchStep))).toBe(false)
    expect(getLaunchText(get(launchStep))).toBe('app.launch.loading')
    putLaunchStep('ready')
    expect(grow).toHaveBeenCalledTimes(1)
  })

  it('does not grow after it is unwatched', () => {
    const grow = vi.fn()
    const stop = watchLaunch(grow)
    vi.advanceTimersByTime(LOGO_MS)
    stop()
    putLaunchStep('ready')
    vi.advanceTimersByTime(LAUNCH_TIMEOUT_MS)
    expect(grow).not.toHaveBeenCalled()
  })

  it('names each startup step in English and Norwegian and goes quiet when ready', () => {
    const steps: LaunchStep[] = ['starting', 'unlocking', 'opening', 'loading', 'setup']
    for (const step of steps) {
      const key = getLaunchText(step)
      expect(key).not.toBeNull()
      expect(en[key!]).toBeTruthy()
      expect(nb[key!]).toBeTruthy()
    }
    expect(getLaunchText('opening')).toBe('app.launch.opening')
    expect(getLaunchText('ready')).toBeNull()
    expect(getLaunchText('locked')).toBeNull()
    expect(LAUNCH_MS).toBeLessThan(300)
  })
})

describe('launch gate with the logo minimum', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    launchStep.set('starting')
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('waits for the logo minimum when the app is ready earlier', () => {
    const grow = vi.fn()
    watchLaunch(grow)
    putLaunchStep('ready')
    vi.advanceTimersByTime(LOGO_MS - 1)
    expect(grow).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(grow).toHaveBeenCalledTimes(1)
  })

  it('grows only once both the logo minimum and ready have happened', () => {
    const grow = vi.fn()
    watchLaunch(grow)
    vi.advanceTimersByTime(LOGO_MS * 2)
    putLaunchStep('opening')
    putLaunchStep('loading')
    expect(grow).not.toHaveBeenCalled()
    putLaunchStep('ready')
    expect(grow).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(LAUNCH_TIMEOUT_MS)
    expect(grow).toHaveBeenCalledTimes(1)
  })

  it('keeps the minimum short enough to feel quick', () => {
    expect(LOGO_MS).toBeGreaterThanOrEqual(500)
    expect(LOGO_MS).toBeLessThanOrEqual(700)
  })
})

describe('frame saves during launch', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    launchSettled.set(false)
  })

  afterEach(() => {
    vi.useRealTimers()
    launchSettled.set(true)
  })

  it('never saves the card or the growing frame, only frames after the grow settles', async () => {
    const { source, handlers } = frameSource()
    const save = vi.fn()
    await watchWindowFrame(source, save)

    handlers.resized[0]()
    handlers.moved[0]()
    vi.advanceTimersByTime(SAVE_MS * 2)
    expect(save).not.toHaveBeenCalled()

    launchSettled.set(true)
    handlers.moved[0]()
    vi.advanceTimersByTime(SAVE_MS)
    expect(save).toHaveBeenCalledTimes(1)
  })

  it('names the launch in English and Norwegian', () => {
    expect(en['app.launch.title']).toBe('Chatnip is loading')
    expect(nb['app.launch.title']).toBe('Chatnip lastes inn')
  })
})

describe('saving the frame robustly', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    launchSettled.set(true)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('saves when the window loses focus, not when it gains it', async () => {
    const { source, focus } = frameSource()
    const save = vi.fn()
    await watchWindowFrame(source, save)
    focus[0]({ payload: true })
    vi.advanceTimersByTime(SAVE_MS)
    expect(save).not.toHaveBeenCalled()
    focus[0]({ payload: false })
    vi.advanceTimersByTime(SAVE_MS)
    expect(save).toHaveBeenCalledTimes(1)
  })

  it('opens saving once the grow finishes, after its duration if the grow never answers, and after a failed grow', async () => {
    launchSettled.set(false)
    putLaunchFrame(() => new Promise(() => {}), 260)
    vi.advanceTimersByTime(260 + SETTLE_MS - 1)
    expect(get(launchSettled)).toBe(false)
    vi.advanceTimersByTime(1)
    expect(get(launchSettled)).toBe(true)

    launchSettled.set(false)
    await putLaunchFrame(() => Promise.resolve(), 260)
    expect(get(launchSettled)).toBe(true)

    launchSettled.set(false)
    await putLaunchFrame(() => Promise.reject(new Error('no window')), 0)
    expect(get(launchSettled)).toBe(true)
  })
})

describe('no ResizeObserver', () => {
  it('leaves no ResizeObserver or size binding in the app sources', () => {
    const sources = import.meta.glob(['/src/**/*.ts', '/src/**/*.svelte', '!/src/lib/generated/**', '!/src/**/*.test.ts'], { query: '?raw', import: 'default', eager: true }) as Record<string, string>
    expect(Object.keys(sources).length).toBeGreaterThan(50)
    for (const [path, source] of Object.entries(sources)) {
      expect(source, path).not.toMatch(/ResizeObserver|bind:(clientWidth|clientHeight|offsetWidth|offsetHeight|contentRect|contentBoxSize|borderBoxSize|devicePixelContentBoxSize)/)
    }
  })

})

describe('window dragging from surfaces', () => {
  function build(html: string): HTMLElement {
    const root = document.createElement('div')
    root.innerHTML = html
    document.body.append(root)
    return root
  }

  it('drags from empty panels and headers but never from controls or text', () => {
    const root = build('<section id="panel"><header id="head"><span id="icon"></span><button id="button"><span id="inside"></span></button><p id="text">Hello</p><input id="field"></header><div role="menuitem" id="item"></div><div data-no-drag id="kept"></div></section>')
    const surface = (id: string) => isSurface(root.querySelector(`#${id}`))
    expect(surface('panel')).toBe(true)
    expect(surface('head')).toBe(true)
    expect(surface('icon')).toBe(true)
    expect(surface('button')).toBe(false)
    expect(surface('inside')).toBe(false)
    expect(surface('text')).toBe(false)
    expect(surface('field')).toBe(false)
    expect(surface('item')).toBe(false)
    expect(surface('kept')).toBe(false)
    root.remove()
  })

  it('a press on a surface starts dragging and a double press toggles maximize', () => {
    const root = build('<div id="pane"></div><button id="go"></button>')
    const startDragging = vi.fn(() => Promise.resolve())
    const toggleMaximize = vi.fn(() => Promise.resolve())
    const stop = watchWindowDrag(document, () => ({ startDragging, toggleMaximize }))
    root.querySelector('#pane')!.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0, detail: 1 }))
    root.querySelector('#go')!.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0, detail: 1 }))
    root.querySelector('#pane')!.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, button: 0 }))
    expect(startDragging).toHaveBeenCalledTimes(1)
    expect(toggleMaximize).toHaveBeenCalledTimes(1)
    stop()
    root.remove()
  })
})
