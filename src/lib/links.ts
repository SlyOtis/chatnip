import * as api from '$lib/tauri/api'
import type { LinkPreview, LinkPreviewMode } from '$lib/tauri/types'

export const MAX_PREVIEWS = 3
export const DISMISSED_KEY = 'chatnip_dismissed_previews'

const URL_PATTERN = /https?:\/\/[^\s<>"'`]+/gi
const TRAILING = /[.,;:!?'"*_~>]+$/

export function isHtml(contentType: string | null | undefined): boolean {
  return contentType === 'html' || contentType === 'text/html'
}

function decodeAmp(text: string): string {
  return text.replace(/&amp;/gi, '&').replace(/&#x2F;/gi, '/').replace(/&#47;/g, '/')
}

export function trimUrl(raw: string): string {
  let url = raw.replace(TRAILING, '')
  const pairs: [string, string][] = [['(', ')'], ['[', ']'], ['{', '}']]
  let changed = true
  while (changed) {
    changed = false
    for (const [open, close] of pairs) {
      if (url.endsWith(close) && url.split(open).length < url.split(close).length) {
        url = url.slice(0, -1).replace(TRAILING, '')
        changed = true
      }
    }
  }
  return url
}

export function stripCode(text: string, html: boolean): string {
  if (html) {
    return text.replace(/<pre\b[\s\S]*?<\/pre>/gi, ' ').replace(/<code\b[\s\S]*?<\/code>/gi, ' ')
  }
  return text.replace(/```[\s\S]*?(```|$)/g, ' ').replace(/`[^`\n]*`/g, ' ')
}

export function extractUrls(content: string | null | undefined, contentType?: string | null): string[] {
  if (!content) return []
  const html = isHtml(contentType)
  const text = stripCode(content, html)
  const found: string[] = []
  if (html) {
    for (const match of text.matchAll(/<a\b[^>]*\bhref\s*=\s*["']([^"']+)["']/gi)) found.push(decodeAmp(match[1]))
  }
  const plain = html ? text.replace(/<[^>]+>/g, ' ') : text
  for (const match of plain.matchAll(URL_PATTERN)) found.push(html ? decodeAmp(match[0]) : match[0])
  const seen = new Set<string>()
  const urls: string[] = []
  for (const raw of found) {
    if (!/^https?:\/\//i.test(raw)) continue
    const url = trimUrl(raw.trim())
    const key = dedupeKey(url)
    if (!key || seen.has(key)) continue
    seen.add(key)
    urls.push(url)
  }
  return urls
}

export function dedupeKey(url: string): string {
  try {
    const parsed = new URL(url)
    parsed.hash = ''
    for (const name of [...parsed.searchParams.keys()]) {
      if (name.startsWith('utm_') || name.startsWith('mc_')) parsed.searchParams.delete(name)
    }
    const host = parsed.host.toLowerCase().replace(/^www\./, '')
    const path = parsed.pathname.replace(/\/+$/, '')
    return `${host}${path}${parsed.search}`
  } catch {
    return ''
  }
}

export function domain(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '')
  } catch {
    return url
  }
}

export function formatClock(seconds: number | null | undefined): string {
  if (!seconds || seconds < 0) return ''
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = Math.floor(seconds % 60)
  const pad = (n: number) => String(n).padStart(2, '0')
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`
}

export function formatSize(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return ''
  const units = ['B', 'KB', 'MB', 'GB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value >= 10 || unit === 0 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`
}

export function isWide(preview: LinkPreview): boolean {
  return preview.kind === 'image' || preview.kind === 'video'
}

export function previewMeta(preview: LinkPreview): string {
  if (preview.kind === 'pull' || preview.kind === 'issue') {
    return [preview.repo && preview.number ? `${preview.repo}#${preview.number}` : preview.repo, preview.state?.replace('_', ' ')].filter(Boolean).join(' · ')
  }
  if (preview.kind === 'repo') return [preview.repo, preview.state].filter(Boolean).join(' · ')
  if (preview.kind === 'pdf') return ['PDF', formatSize(preview.size)].filter(Boolean).join(' · ')
  if (preview.kind === 'image') {
    const dims = preview.image_width && preview.image_height ? `${preview.image_width}×${preview.image_height}` : ''
    return [dims, formatSize(preview.size)].filter(Boolean).join(' · ')
  }
  return [preview.author, formatClock(preview.duration)].filter(Boolean).join(' · ')
}

const inflight = new Map<string, Promise<LinkPreview | null>>()

export function getPreview(url: string): Promise<LinkPreview | null> {
  const known = inflight.get(url)
  if (known) return known
  const request = api.getLinkPreview(url).catch((error) => {
    inflight.delete(url)
    throw error
  })
  inflight.set(url, request)
  return request
}

let mode: Promise<LinkPreviewMode> | null = null

export function getPreviewMode(): Promise<LinkPreviewMode> {
  mode ??= api
    .getAppSettings()
    .then((settings) => (settings['links.previews'] as LinkPreviewMode) || 'on')
    .catch(() => 'on' as LinkPreviewMode)
  return mode
}

export function putPreviewMode(value: LinkPreviewMode) {
  mode = Promise.resolve(value)
  inflight.clear()
}

export function loadDismissed(): Set<string> {
  try {
    const parsed = JSON.parse(localStorage.getItem(DISMISSED_KEY) ?? '[]')
    return new Set(Array.isArray(parsed) ? parsed.filter((entry) => typeof entry === 'string') : [])
  } catch {
    return new Set()
  }
}

export function dismiss(url: string): Set<string> {
  const dismissed = loadDismissed()
  dismissed.add(dedupeKey(url))
  try {
    localStorage.setItem(DISMISSED_KEY, JSON.stringify([...dismissed].slice(-500)))
  } catch {}
  return dismissed
}

export function previewUrls(content: string | null | undefined, contentType: string | null | undefined, dismissed: Set<string>, max = MAX_PREVIEWS): string[] {
  return extractUrls(content, contentType)
    .filter((url) => !dismissed.has(dedupeKey(url)))
    .slice(0, max)
}
