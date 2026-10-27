import type { Window } from '@tauri-apps/api/window'
import { debounce } from '$lib/debounce'
import type { MessageKey } from '$lib/i18n.svelte'
import { get, writable } from 'svelte/store'

export const LAUNCH_MS = 260
export const LAUNCH_TIMEOUT_MS = 8000
export const LOGO_MS = 600
export const SAVE_MS = 500

export type FrameSource = Pick<Window, 'onMoved' | 'onResized' | 'onFocusChanged'>

export const launchSettled = writable(false)

export function getReducedMotion(media: Pick<typeof globalThis, 'matchMedia'> | undefined = globalThis): boolean {
  return !!media?.matchMedia?.('(prefers-reduced-motion: reduce)').matches
}

export function getLaunchDuration(reduced: boolean): number {
  return reduced ? 0 : LAUNCH_MS
}

export async function watchWindowFrame(source: FrameSource, save: () => unknown, wait = SAVE_MS): Promise<() => void> {
  const queue = debounce(() => {
    if (get(launchSettled)) save()
  }, wait)
  const stops = await Promise.all([
    source.onMoved(() => queue()),
    source.onResized(() => queue()),
    source.onFocusChanged(({ payload }) => {
      if (!payload) queue()
    })
  ])
  return () => stops.forEach((stop) => stop())
}

export const SETTLE_MS = 150

export function putLaunchFrame(grow: () => Promise<unknown>, duration: number): Promise<void> {
  const settle = setTimeout(() => launchSettled.set(true), duration + SETTLE_MS)
  return grow()
    .catch(() => {})
    .then(() => {
      clearTimeout(settle)
      launchSettled.set(true)
    })
}

export function watchWindowActivity(target: Pick<typeof globalThis, 'addEventListener' | 'removeEventListener'>, doc: Document): () => void {
  const root = doc.documentElement
  const update = () => {
    if (doc.hidden) root.dataset.idle = ''
    else delete root.dataset.idle
  }
  target.addEventListener('focus', update)
  target.addEventListener('blur', update)
  doc.addEventListener('visibilitychange', update)
  update()
  return () => {
    target.removeEventListener('focus', update)
    target.removeEventListener('blur', update)
    doc.removeEventListener('visibilitychange', update)
  }
}

export type LaunchStep = 'starting' | 'unlocking' | 'opening' | 'loading' | 'setup' | 'locked' | 'ready'

export const LAUNCH_TEXT: Record<LaunchStep, MessageKey | null> = {
  starting: 'app.launch.starting',
  unlocking: 'app.launch.unlocking',
  opening: 'app.launch.opening',
  loading: 'app.launch.loading',
  setup: 'app.launch.setup',
  locked: null,
  ready: null
}

export const launchStep = writable<LaunchStep>('starting')

export function getLaunchText(step: LaunchStep): MessageKey | null {
  return LAUNCH_TEXT[step]
}

export function getLaunchReady(step: LaunchStep): boolean {
  return step === 'ready' || step === 'locked'
}

export function putLaunchStep(step: LaunchStep): void {
  launchStep.update((current) => (current === 'ready' ? current : step))
}

export function watchLaunch(grow: () => unknown, timeout = LAUNCH_TIMEOUT_MS, minimum = LOGO_MS): () => void {
  let grown = false
  let shown = false
  let ready = false
  const once = () => {
    if (grown) return
    grown = true
    grow()
  }
  const settle = () => {
    if (shown && ready) once()
  }
  const logo = setTimeout(() => {
    shown = true
    settle()
  }, minimum)
  const timer = setTimeout(once, timeout)
  const stop = launchStep.subscribe((step) => {
    ready = getLaunchReady(step)
    settle()
  })
  return () => {
    clearTimeout(logo)
    clearTimeout(timer)
    stop()
  }
}

export interface ElementSize {
  width: number
  height: number
}

export function elementSize(node: HTMLElement, report: (size: ElementSize) => void) {
  let current = report
  let last = ''
  let frame = 0
  const measure = () => {
    cancelAnimationFrame(frame)
    frame = requestAnimationFrame(() => {
      const size = { width: node.clientWidth, height: node.clientHeight }
      const key = `${size.width}x${size.height}`
      if (key === last) return
      last = key
      current(size)
    })
  }
  const observer = new MutationObserver(measure)
  observer.observe(node, { childList: true, subtree: true, characterData: true, attributes: true })
  node.addEventListener('load', measure, true)
  window.addEventListener('resize', measure)
  document.addEventListener('transitionend', measure)
  document.addEventListener('pointerup', measure)
  measure()
  return {
    update(next: (size: ElementSize) => void) {
      current = next
    },
    destroy() {
      cancelAnimationFrame(frame)
      observer.disconnect()
      node.removeEventListener('load', measure, true)
      window.removeEventListener('resize', measure)
      document.removeEventListener('transitionend', measure)
      document.removeEventListener('pointerup', measure)
    }
  }
}

export const PULSE_MS = 1500
export const PULSE_CLASS = 'reveal-pulse'

let pulseTimer: ReturnType<typeof setTimeout> | null = null

export function pulse(element: Element | null | undefined): void {
  if (!element) return
  element.ownerDocument.querySelectorAll(`.${PULSE_CLASS}`).forEach((el) => el.classList.remove(PULSE_CLASS))
  void (element as HTMLElement).offsetWidth
  element.classList.add(PULSE_CLASS)
  if (pulseTimer) clearTimeout(pulseTimer)
  pulseTimer = setTimeout(() => element.classList.remove(PULSE_CLASS), PULSE_MS)
}

export const INTERACTIVE = [
  'a',
  'button',
  'input',
  'textarea',
  'select',
  'option',
  'label',
  'summary',
  'video',
  'audio',
  'canvas',
  'iframe',
  '[contenteditable]:not([contenteditable="false"])',
  '[draggable="true"]',
  '[tabindex]:not([tabindex="-1"])',
  '[role="button"]',
  '[role="link"]',
  '[role="menuitem"]',
  '[role="menuitemcheckbox"]',
  '[role="menuitemradio"]',
  '[role="option"]',
  '[role="tab"]',
  '[role="checkbox"]',
  '[role="radio"]',
  '[role="switch"]',
  '[role="slider"]',
  '[role="textbox"]',
  '[role="combobox"]',
  '[role="separator"]',
  '.cm-editor',
  '[data-no-drag]'
].join(', ')

function ownText(element: Element): boolean {
  return Array.from(element.childNodes).some((node) => node.nodeType === Node.TEXT_NODE && !!node.textContent?.trim())
}

export function isSurface(target: EventTarget | null, x = 0, y = 0): boolean {
  if (!(target instanceof Element)) return false
  if (target.closest(INTERACTIVE)) return false
  if (ownText(target)) return false
  if (target instanceof HTMLElement && (x > target.getBoundingClientRect().left + target.clientWidth || y > target.getBoundingClientRect().top + target.clientHeight)) return false
  return true
}

export function watchWindowDrag(doc: Document, current: () => Pick<Window, 'startDragging' | 'toggleMaximize'>): () => void {
  const down = (event: MouseEvent) => {
    if (event.button !== 0 || event.defaultPrevented || event.shiftKey || event.detail > 1) return
    if (!isSurface(event.target, event.clientX, event.clientY)) return
    event.preventDefault()
    current().startDragging().catch(() => {})
  }
  const twice = (event: MouseEvent) => {
    if (event.button !== 0 || !isSurface(event.target, event.clientX, event.clientY)) return
    current().toggleMaximize().catch(() => {})
  }
  doc.addEventListener('mousedown', down)
  doc.addEventListener('dblclick', twice)
  return () => {
    doc.removeEventListener('mousedown', down)
    doc.removeEventListener('dblclick', twice)
  }
}
