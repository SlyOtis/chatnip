import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
  convertFileSrc: vi.fn((path: string) => `asset://localhost/${path}`)
}))

import { invoke } from '@tauri-apps/api/core'
import type { LinkPreview } from '$lib/tauri/types'
import { dedupeKey, dismiss, domain, extractUrls, formatClock, formatSize, getPreview, isWide, loadDismissed, previewMeta, previewUrls, trimUrl } from './links'

function preview(overrides: Partial<LinkPreview> = {}): LinkPreview {
  return {
    url: 'https://github.com/a/b/pull/4',
    kind: 'article',
    site_name: null,
    title: null,
    description: null,
    image_url: null,
    image: null,
    image_width: null,
    image_height: null,
    favicon_url: null,
    favicon: null,
    theme_color: null,
    author: null,
    duration: null,
    state: null,
    repo: null,
    number: null,
    file_name: null,
    size: null,
    ...overrides
  }
}

describe('extractUrls', () => {
  it('drops trailing punctuation and keeps balanced parentheses', () => {
    expect(extractUrls('see https://a.no/x. and (https://b.no/y), or https://en.wikipedia.org/wiki/Fjord_(disambiguation)!')).toEqual([
      'https://a.no/x',
      'https://b.no/y',
      'https://en.wikipedia.org/wiki/Fjord_(disambiguation)'
    ])
    expect(trimUrl('https://a.no/x?"')).toBe('https://a.no/x')
  })

  it('skips urls inside code and dedupes variants', () => {
    const text = 'run `curl https://code.no/a` then\n```\nhttps://fenced.no/b\n```\nhttps://www.a.no/x/ and https://a.no/x?utm_source=mail and https://a.no/x#top'
    expect(extractUrls(text)).toEqual(['https://www.a.no/x/'])
  })

  it('reads anchors and text from html but not code blocks', () => {
    const html = '<p>Look <a href="https://github.com/a/b?x=1&amp;y=2">here</a> and https://youtu.be/abcdefg.</p><pre>https://pre.no/</pre><code>https://code.no/</code>'
    expect(extractUrls(html, 'html')).toEqual(['https://github.com/a/b?x=1&y=2', 'https://youtu.be/abcdefg'])
  })

  it('handles empty content', () => {
    expect(extractUrls('')).toEqual([])
    expect(extractUrls(null)).toEqual([])
    expect(extractUrls('mailto:a@b.no ftp://x.no')).toEqual([])
  })
})

describe('previewUrls', () => {
  beforeEach(() => {
    const store = new Map<string, string>()
    vi.stubGlobal('localStorage', { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => store.set(k, v), removeItem: (k: string) => store.delete(k) })
  })

  it('caps at three and hides dismissed links', () => {
    const text = 'https://a.no https://b.no https://c.no https://d.no'
    expect(previewUrls(text, 'text', new Set())).toEqual(['https://a.no', 'https://b.no', 'https://c.no'])
    const dismissed = dismiss('https://www.b.no/')
    expect(loadDismissed().has(dedupeKey('https://b.no'))).toBe(true)
    expect(previewUrls(text, 'text', dismissed)).toEqual(['https://a.no', 'https://c.no', 'https://d.no'])
  })
})

describe('formatting', () => {
  it('formats domains, durations and sizes', () => {
    expect(domain('https://www.northwind.example/a')).toBe('northwind.example')
    expect(domain('nope')).toBe('nope')
    expect(formatClock(213)).toBe('3:33')
    expect(formatClock(3723)).toBe('1:02:03')
    expect(formatClock(null)).toBe('')
    expect(formatSize(512)).toBe('512 B')
    expect(formatSize(1536)).toBe('1.5 KB')
    expect(formatSize(20 * 1024 * 1024)).toBe('20 MB')
  })

  it('describes previews by kind', () => {
    expect(previewMeta(preview({ kind: 'pull', repo: 'a/b', number: 4, state: 'merged' }))).toBe('a/b#4 · merged')
    expect(previewMeta(preview({ kind: 'issue', repo: 'a/b', number: 1, state: 'not_planned' }))).toBe('a/b#1 · not planned')
    expect(previewMeta(preview({ kind: 'pdf', size: 2048 }))).toBe('PDF · 2.0 KB')
    expect(previewMeta(preview({ kind: 'video', author: 'Rick', duration: 213 }))).toBe('Rick · 3:33')
    expect(previewMeta(preview({ kind: 'image', image_width: 10, image_height: 20 }))).toBe('10×20')
    expect(isWide(preview({ kind: 'video' }))).toBe(true)
    expect(isWide(preview({ kind: 'article' }))).toBe(false)
  })
})

describe('getPreview', () => {
  it('shares one request per url', async () => {
    const mocked = vi.mocked(invoke)
    mocked.mockClear()
    await Promise.all([getPreview('https://same.no'), getPreview('https://same.no')])
    expect(mocked).toHaveBeenCalledTimes(1)
    expect(mocked).toHaveBeenCalledWith('get_link_preview', { url: 'https://same.no' })
  })
})
