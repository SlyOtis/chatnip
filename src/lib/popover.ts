import { writable } from 'svelte/store'
import { isTyping } from '$lib/shortcuts'

export interface Box {
  left: number
  top: number
  width: number
  height: number
}

export interface Placement {
  left: number
  top: number
  maxWidth: number
  maxHeight: number
}

export type Side = 'below' | 'above'
export type Align = 'start' | 'end'

export interface PlaceOptions {
  gap?: number
  margin?: number
  side?: Side
  align?: Align
}

export type PopupKind = 'popup' | 'dialog'

export interface AnchorOptions extends PlaceOptions {
  at: Box | (() => Box | null) | null
  onclose?: () => void
  kind?: PopupKind
}

export interface DismissOptions {
  onclose: () => void
  kind?: PopupKind
}

interface Entry {
  node: HTMLElement
  kind: PopupKind
  opener: Element | null
  close: () => void
}

export function boxOf(target: EventTarget | Element | null): Box | null {
  if (!target || !(target instanceof Element)) return null
  const rect = target.getBoundingClientRect()
  return { left: rect.left, top: rect.top, width: rect.width, height: rect.height }
}

export function pointBox(x: number, y: number): Box {
  return { left: x, top: y, width: 0, height: 0 }
}

export const sheetAnchor = writable<Box | null>(null)

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [role="menuitem"], [role="option"], [tabindex]:not([tabindex="-1"])'

export function navigable(node: HTMLElement) {
  const items = () => [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.offsetParent !== null || el === document.activeElement)
  const focusFirst = () => {
    if (node.contains(document.activeElement) && document.activeElement !== node) return
    if (isTyping(document.activeElement)) return
    const first = items()[0]
    ;(first ?? node).focus({ preventScroll: true })
  }
  const keydown = (e: KeyboardEvent) => {
    if (e.altKey || e.metaKey || e.ctrlKey) return
    const vertical = e.key === 'ArrowDown' || e.key === 'ArrowUp'
    const edge = e.key === 'Home' || e.key === 'End'
    if (!vertical && !edge) return
    if (edge && isTyping(e.target)) return
    const list = items()
    if (!list.length) return
    const at = list.indexOf(document.activeElement as HTMLElement)
    const next = e.key === 'Home' ? 0 : e.key === 'End' ? list.length - 1 : e.key === 'ArrowDown' ? (at + 1) % list.length : (at - 1 + list.length) % list.length
    e.preventDefault()
    e.stopPropagation()
    list[next].focus({ preventScroll: true })
    list[next].scrollIntoView({ block: 'nearest' })
  }
  node.addEventListener('keydown', keydown)
  let frame = requestAnimationFrame(() => {
    frame = requestAnimationFrame(focusFirst)
  })
  return {
    destroy() {
      cancelAnimationFrame(frame)
      node.removeEventListener('keydown', keydown)
    }
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(min, value), Math.max(min, max))
}

export function placePopover(anchor: Box, size: { width: number; height: number }, viewport: { width: number; height: number }, options: PlaceOptions = {}): Placement {
  const { gap = 6, margin = 8, side = 'below', align = 'end' } = options
  const maxWidth = Math.max(0, viewport.width - margin * 2)
  const fullHeight = Math.max(0, viewport.height - margin * 2)
  const width = Math.min(size.width, maxWidth)
  const space: Record<Side, number> = {
    below: viewport.height - margin - (anchor.top + anchor.height + gap),
    above: anchor.top - gap - margin
  }
  const order: Side[] = side === 'below' ? ['below', 'above'] : ['above', 'below']
  const fit = order.find((candidate) => size.height <= space[candidate])
  let top: number
  let maxHeight: number
  if (fit) {
    maxHeight = Math.min(space[fit], fullHeight)
    top = fit === 'below' ? anchor.top + anchor.height + gap : anchor.top - gap - size.height
  } else {
    maxHeight = fullHeight
    top = anchor.top + anchor.height + gap
  }
  const height = Math.min(size.height, maxHeight)
  let left = align === 'end' ? anchor.left + anchor.width - width : anchor.left
  if (align === 'end' && left < margin) left = anchor.left
  if (align === 'start' && left + width > viewport.width - margin) left = anchor.left + anchor.width - width
  return {
    left: Math.round(clamp(left, margin, viewport.width - margin - width)),
    top: Math.round(clamp(top, margin, viewport.height - margin - height)),
    maxWidth: Math.round(maxWidth),
    maxHeight: Math.round(maxHeight)
  }
}

const stack: Entry[] = []
let lastPointer: { target: Element | null; button: number } = { target: null, button: 0 }
let listening = false

function listen() {
  if (listening || typeof window === 'undefined') return
  listening = true
  window.addEventListener(
    'pointerdown',
    (event) => {
      const target = event.target instanceof Element ? event.target : null
      lastPointer = { target, button: event.button }
      if (!target) return
      for (const entry of [...stack].reverse()) {
        if (entry.node.contains(target) || entry.opener?.contains(target)) break
        entry.close()
      }
    },
    true
  )
  const scrolled = (event: Event) => {
    const target = event.target instanceof Node ? event.target : null
    for (const entry of [...stack].reverse()) {
      if (target && entry.node.contains(target)) break
      if (entry.kind === 'popup') entry.close()
    }
  }
  window.addEventListener('wheel', scrolled, { capture: true, passive: true })
  window.addEventListener('touchmove', scrolled, { capture: true, passive: true })
  window.addEventListener(
    'keydown',
    (event) => {
      if (event.key !== 'Escape' || stack.length === 0) return
      event.preventDefault()
      event.stopPropagation()
      stack[stack.length - 1].close()
    },
    true
  )
}

function openerOf(): Element | null {
  if (lastPointer.button !== 0 || !lastPointer.target) return null
  return lastPointer.target.closest('button, [role="button"], a, summary')
}

function register(node: HTMLElement, onclose: () => void, kind: PopupKind, owners: Entry[] = []): Entry {
  listen()
  if (kind === 'popup') {
    for (const entry of [...stack]) if (entry.kind === 'popup' && !owners.includes(entry) && !entry.node.contains(node)) entry.close()
  } else {
    for (const entry of [...stack]) if (entry.kind === 'popup') entry.close()
  }
  const entry: Entry = { node, kind, opener: openerOf(), close: onclose }
  stack.push(entry)
  return entry
}

function unregister(entry: Entry) {
  const index = stack.indexOf(entry)
  if (index >= 0) stack.splice(index, 1)
}

export function portal(node: HTMLElement) {
  document.body.appendChild(node)
  return {
    destroy() {
      node.remove()
    }
  }
}

export function dismissable(node: HTMLElement, options: DismissOptions) {
  let current = options
  const entry = register(node, () => current.onclose(), options.kind ?? 'dialog')
  return {
    update(next: DismissOptions) {
      current = next
    },
    destroy() {
      unregister(entry)
    }
  }
}

export function anchored(node: HTMLElement, options: AnchorOptions) {
  if (options.at === null) return {}
  let current = options
  let frame = 0
  const keys = navigable(node)
  const owners = stack.filter((entry) => entry.node.contains(node))
  document.body.appendChild(node)
  node.style.position = 'fixed'
  node.style.zIndex = '1000'
  const place = () => {
    const box = typeof current.at === 'function' ? current.at() : current.at
    if (!box) return
    node.style.maxHeight = ''
    node.style.maxWidth = ''
    const size = { width: node.offsetWidth, height: node.offsetHeight }
    const spot = placePopover(box, size, { width: window.innerWidth, height: window.innerHeight }, current)
    node.style.left = `${spot.left}px`
    node.style.top = `${spot.top}px`
    node.style.maxWidth = `${spot.maxWidth}px`
    node.style.maxHeight = `${spot.maxHeight}px`
    node.style.overflowY = size.height > spot.maxHeight ? 'auto' : ''
  }
  const schedule = () => {
    cancelAnimationFrame(frame)
    frame = requestAnimationFrame(place)
  }
  const entry = current.onclose ? register(node, () => current.onclose?.(), current.kind ?? 'popup', owners) : null
  node.style.visibility = 'hidden'
  place()
  frame = requestAnimationFrame(() => {
    place()
    node.style.visibility = ''
  })
  const observer = new MutationObserver(schedule)
  observer.observe(node, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ['class', 'hidden'] })
  node.addEventListener('load', schedule, true)
  node.addEventListener('transitionend', schedule)
  window.addEventListener('resize', schedule)
  window.addEventListener('scroll', schedule, true)
  return {
    update(next: AnchorOptions) {
      current = next
      place()
    },
    destroy() {
      cancelAnimationFrame(frame)
      observer.disconnect()
      window.removeEventListener('resize', schedule)
      window.removeEventListener('scroll', schedule, true)
      keys.destroy()
      if (entry) unregister(entry)
      node.remove()
    }
  }
}
