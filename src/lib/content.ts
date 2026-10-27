import type { Person } from './tauri/types'
import { PERSON_LINK, PERSON_NAME, personHref, personOf } from './tauri/people'
import { parseMetadata } from './messageKind'
import { trimUrl } from './links'
import { classHighlighter, highlightCode } from '@lezer/highlight'
import { javascriptLanguage, jsxLanguage, tsxLanguage, typescriptLanguage } from '@codemirror/lang-javascript'
import { jsonLanguage } from '@codemirror/lang-json'
import { htmlLanguage } from '@codemirror/lang-html'
export { isEmojiOnly } from './messageKind'

export interface Mention {
  id: string
  name: string
}

export function mentionsOf(metadata: string | null | undefined): Mention[] {
  const raw = (parseMetadata(metadata).structure as { mentions?: unknown } | undefined)?.mentions
  return Array.isArray(raw) ? raw.filter((m) => m && typeof m.name === 'string').map((m) => ({ id: String(m.id ?? ''), name: m.name })) : []
}

export function linkMentions(html: string, mentions: Mention[], people: Person[]): string {
  if (!/class="msg-mention"|<at[\s>]/i.test(html)) return html
  const div = document.createElement('div')
  div.innerHTML = html
  div.querySelectorAll('.msg-mention, at').forEach((element) => {
    const label = element.textContent ?? ''
    const name = label.replace(/^@/, '').trim().toLowerCase()
    const mention = mentions.find((m) => m.name.trim().toLowerCase() === name)
    const person = personOf(people, { user_id: mention?.id, external_id: mention?.id }) ?? people.find((p) => p.display_name.trim().toLowerCase() === name) ?? null
    const link = document.createElement(person ? 'a' : 'strong')
    link.className = person ? PERSON_LINK : PERSON_NAME
    if (person) {
      link.setAttribute('href', personHref(person.id))
      link.setAttribute('data-person', person.id)
    }
    link.textContent = label
    element.replaceWith(link)
  })
  return div.innerHTML
}

export const URL_REGEX = /https?:\/\/[^\s<>"]+/g

export const QUOTE_SELECTOR = 'blockquote, .gmail_quote, #divRplyFwdMsg, #appendonsend, [id^="x_divRplyFwdMsg"], .moz-cite-prefix, .yahoo_quoted, .protonmail_quote'

const COPY_BUTTON = `<button class="code-copy-btn" onclick="navigator.clipboard.writeText(this.closest('.code-block-wrapper').querySelector('code').textContent)"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg></button>`

export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

export function sanitizeHtml(html: string): string {
  const div = document.createElement('div')
  div.innerHTML = html
  div.querySelectorAll('script').forEach((s) => s.remove())
  div.querySelectorAll('*').forEach((el) => {
    Array.from(el.attributes).forEach((attr) => {
      if (attr.name.startsWith('on')) el.removeAttribute(attr.name)
    })
  })
  return div.innerHTML
}

const CODE_PARSERS: Record<string, typeof javascriptLanguage.parser> = {
  js: javascriptLanguage.parser,
  javascript: javascriptLanguage.parser,
  mjs: javascriptLanguage.parser,
  jsx: jsxLanguage.parser,
  ts: typescriptLanguage.parser,
  typescript: typescriptLanguage.parser,
  tsx: tsxLanguage.parser,
  json: jsonLanguage.parser,
  html: htmlLanguage.parser,
  svelte: htmlLanguage.parser,
  vue: htmlLanguage.parser
}

export function highlighted(lang: string, code: string): string {
  const parser = CODE_PARSERS[lang.trim().toLowerCase()]
  if (!parser) return escapeHtml(code)
  let out = ''
  highlightCode(
    code,
    parser.parse(code),
    classHighlighter,
    (text, classes) => {
      out += classes ? `<span class="${classes}">${escapeHtml(text)}</span>` : escapeHtml(text)
    },
    () => {
      out += '\n'
    }
  )
  return out
}

export function codeBlock(lang: string, code: string): string {
  const header = lang
    ? `<div class="code-block-header"><span class="code-lang">${escapeHtml(lang)}</span>${COPY_BUTTON}</div>`
    : `<div class="code-block-header">${COPY_BUTTON}</div>`
  return `<div class="code-block-wrapper">${header}<pre class="code-block"><code>${highlighted(lang, code.replace(/\n+$/, ''))}</code></pre></div>`
}

function anchor(href: string, label: string): string {
  return `<a href="${escapeHtml(href)}" target="_blank" rel="noopener noreferrer" class="msg-link">${label}</a>`
}

export function renderInline(text: string): string {
  const slots: string[] = []
  const hold = (html: string) => `\uE000${slots.push(html) - 1}\uE000`
  let out = text.replace(/`([^`\n]+)`/g, (_m, code: string) => hold(`<code class="inline-code">${escapeHtml(code)}</code>`))
  out = out.replace(/\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)/g, (_m, label: string, href: string) => hold(anchor(href, escapeHtml(label))))
  out = out.replace(URL_REGEX, (raw) => {
    const url = trimUrl(raw)
    return hold(anchor(url, escapeHtml(url))) + raw.slice(url.length)
  })
  out = escapeHtml(out)
  out = out.replace(/(^|[^\w.@&])@([\p{L}\p{N}_][\p{L}\p{N}_-]*)/gu, (_m, before: string, name: string) => `${before}<span class="msg-mention">@${name}</span>`)
  out = out.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
  out = out.replace(/(?<!\*)\*([^*\n]+)\*(?!\*)/g, '<strong>$1</strong>')
  out = out.replace(/(?<![\p{L}\p{N}])_([^_\n]+)_(?![\p{L}\p{N}])/gu, '<em>$1</em>')
  out = out.replace(/~~?([^~\n]+)~~?/g, '<del>$1</del>')
  return out.replace(/\uE000(\d+)\uE000/g, (_m, i: string) => slots[Number(i)])
}

export function parseMarkdown(text: string): string {
  if (!text) return ''
  const lines = text.split('\n')
  const parts: string[] = []
  let list: 'ul' | 'ol' | null = null
  let pendingBreak = false
  const closeList = () => {
    if (list) parts.push(list === 'ul' ? '</ul>' : '</ol>')
    list = null
  }
  const openList = (kind: 'ul' | 'ol') => {
    if (list === kind) return
    closeList()
    parts.push(kind === 'ul' ? '<ul class="msg-list">' : '<ol class="msg-list-ordered">')
    list = kind
  }
  const pushLine = (html: string) => {
    if (pendingBreak) parts.push('<br>')
    parts.push(html)
    pendingBreak = true
  }
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const fence = line.match(/^```(\w*)\s*$/)
    if (fence) {
      const end = lines.findIndex((l, j) => j > i && /^```\s*$/.test(l))
      if (end > i) {
        closeList()
        parts.push(codeBlock(fence[1], lines.slice(i + 1, end).join('\n')))
        pendingBreak = false
        i = end
        continue
      }
    }
    const heading = line.match(/^(#{1,6})\s+(.+?)\s*#*\s*$/)
    if (heading) {
      closeList()
      const level = Math.min(heading[1].length, 3)
      parts.push(`<h${level} class="msg-heading msg-heading-${heading[1].length}">${renderInline(heading[2])}</h${level}>`)
      pendingBreak = false
      continue
    }
    if (/^\s*(-{3,}|\*{3,}|_{3,})\s*$/.test(line)) {
      closeList()
      parts.push('<hr class="msg-rule">')
      pendingBreak = false
      continue
    }
    if (/^\s*\|.*\|\s*$/.test(line) && /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$/.test(lines[i + 1] ?? '')) {
      closeList()
      const cells = (row: string) => row.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((cell) => renderInline(cell.trim()))
      const head = cells(line)
      const body: string[][] = []
      let j = i + 2
      while (j < lines.length && /^\s*\|.*\|\s*$/.test(lines[j])) {
        body.push(cells(lines[j]))
        j += 1
      }
      parts.push(`<table class="msg-table"><thead><tr>${head.map((cell) => `<th>${cell}</th>`).join('')}</tr></thead><tbody>${body.map((row) => `<tr>${row.map((cell) => `<td>${cell}</td>`).join('')}</tr>`).join('')}</tbody></table>`)
      pendingBreak = false
      i = j - 1
      continue
    }
    const bullet = line.match(/^\s*[-*]\s+(.+)$/)
    const ordered = line.match(/^\s*\d+[.)]\s+(.+)$/)
    if (bullet) {
      openList('ul')
      const task = bullet[1].match(/^\[( |x|X)\]\s+(.+)$/)
      if (task) {
        const done = task[1] !== ' '
        parts.push(`<li class="msg-task" data-done="${done}"><span class="msg-check" aria-hidden="true">${done ? '☑' : '☐'}</span>${renderInline(task[2])}</li>`)
      } else {
        parts.push(`<li>${renderInline(bullet[1])}</li>`)
      }
      pendingBreak = false
      continue
    }
    if (ordered) {
      openList('ol')
      parts.push(`<li>${renderInline(ordered[1])}</li>`)
      pendingBreak = false
      continue
    }
    if (list) {
      closeList()
      pendingBreak = false
    }
    const quote = line.match(/^>\s?(.*)$/)
    if (quote) {
      parts.push(`<blockquote class="msg-blockquote">${renderInline(quote[1])}</blockquote>`)
      pendingBreak = false
      continue
    }
    pushLine(renderInline(line))
  }
  closeList()
  return parts.join('')
}

export interface QuoteSplit {
  visible: string
  quoted: string
  signature: string
}

const TEXT_QUOTE_HEADER = /^(on .+wrote:|den .+ skrev .+:|.+ skrev .+:|-{2,}\s*original message\s*-{2,}|-{2,}\s*forwarded message\s*-{2,}|from: .+)$/i

function foldPoint(doc: Document, selector: string): Element | null {
  const found = doc.body.querySelector(selector)
  if (!found) return null
  let start: Element = found
  for (;;) {
    const previous = start.previousElementSibling
    if (previous && (previous.tagName === 'HR' || previous.classList.contains('gmail_signature_prefix') || (!previous.matches('img, table, video, picture, svg') && !previous.querySelector('img, table, video, picture, svg') && !(previous.textContent ?? '').replace(/\u00a0/g, '').trim()))) {
      start = previous
      continue
    }
    const parent = start.parentElement
    if (parent && parent !== doc.body && parent.firstElementChild === start && !(parent.firstChild?.nodeType === 3 && parent.firstChild.textContent?.trim())) {
      start = parent
      continue
    }
    return start
  }
}

function extractFrom(doc: Document, start: Element): string {
  const range = doc.createRange()
  range.setStartBefore(start)
  range.setEndAfter(doc.body.lastChild ?? start)
  const holder = doc.createElement('div')
  holder.appendChild(range.extractContents())
  return holder.innerHTML
}

export function splitQuoted(content: string | null | undefined, isHtml: boolean): QuoteSplit {
  const text = content || ''
  if (isHtml) {
    const doc = new DOMParser().parseFromString(text, 'text/html')
    const quote = foldPoint(doc, QUOTE_SELECTOR)
    const quoted = quote ? extractFrom(doc, quote) : ''
    if (quote && !doc.body.textContent?.trim()) return { visible: text, quoted: '', signature: '' }
    const mark = foldPoint(doc, SIGNATURE_SELECTOR)
    const signature = mark ? extractFrom(doc, mark) : ''
    if (!quoted && !signature) return { visible: text, quoted: '', signature: '' }
    return { visible: doc.body.innerHTML, quoted, signature }
  }
  const lines = text.split('\n')
  const start = lines.findIndex((line, index) => index > 0 && (TEXT_QUOTE_HEADER.test(line.trim()) || (line.startsWith('>') && lines.slice(0, index).some((l) => l.trim() && !l.startsWith('>')))))
  const body = start > 0 ? lines.slice(0, start) : lines
  const quoted = start > 0 ? lines.slice(start).join('\n') : ''
  const cut = body.findIndex((line, index) => index > 0 && /^--\s?$/.test(line))
  const visible = (cut > 0 ? body.slice(0, cut) : body).join('\n').trimEnd()
  const signature = cut > 0 ? body.slice(cut + 1).join('\n').trim() : ''
  if (!visible.trim()) return { visible: text, quoted: '', signature: '' }
  return { visible: start > 0 || cut > 0 ? visible : text, quoted, signature }
}

export const SIGNATURE_SELECTOR = '.gmail_signature, .signature, .moz-signature, #Signature, [id^="x_Signature"], #signature'

export interface PreparedMail {
  html: string
  remote: number
  folded: number
  designed: boolean
  foldable: boolean
}

export interface MailOptions {
  cids?: Record<string, string>
  remote?: boolean
}

const REMOTE_URL = /^(https?:)?\/\//i
const REMOTE_CSS = /url\(\s*(['"]?)(https?:)?\/\/[^)]*\)/gi
const PAINT = /(^|[;{\s])background(-color|-image)?\s*:/i

export function prepareMail(html: string, options: MailOptions = {}): PreparedMail {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  doc.querySelectorAll('script, iframe, frame, frameset, object, embed, applet, form, base, meta, link:not([rel~="stylesheet"])').forEach((el) => el.remove())
  let remote = 0
  const styles = [...doc.querySelectorAll('style')]
  const designed =
    !!doc.querySelector('[bgcolor], [background], font[color], body[text], table[width]') ||
    [...doc.querySelectorAll('[style]')].some((el) => PAINT.test(el.getAttribute('style') ?? '')) ||
    styles.some((s) => PAINT.test(s.textContent ?? ''))
  doc.querySelectorAll('*').forEach((el) => {
    for (const attr of [...el.attributes]) {
      const value = attr.value.trim().toLowerCase()
      if (attr.name.startsWith('on') || ((attr.name === 'href' || attr.name === 'src' || attr.name === 'action' || attr.name === 'xlink:href') && (value.startsWith('javascript:') || value.startsWith('vbscript:')))) {
        el.removeAttribute(attr.name)
      }
    }
    const background = el.getAttribute('background')
    if (background && REMOTE_URL.test(background)) {
      remote += 1
      if (!options.remote) el.removeAttribute('background')
    }
    const style = el.getAttribute('style')
    if (style && REMOTE_CSS.test(style)) {
      remote += 1
      if (!options.remote) el.setAttribute('style', style.replace(REMOTE_CSS, 'none'))
    }
    REMOTE_CSS.lastIndex = 0
  })
  doc.querySelectorAll('img').forEach((img) => {
    const src = img.getAttribute('src') ?? ''
    if (src.toLowerCase().startsWith('cid:')) {
      const target = options.cids?.[src.slice(4)] ?? options.cids?.[src.slice(4).replace(/^<|>$/g, '')]
      if (target) img.setAttribute('src', target)
      else img.removeAttribute('src')
      return
    }
    if (REMOTE_URL.test(src)) {
      remote += 1
      if (!options.remote) {
        img.removeAttribute('src')
        img.removeAttribute('srcset')
        img.setAttribute('data-blocked', '')
      }
    }
  })
  if (!options.remote) {
    doc.querySelectorAll('link[rel~="stylesheet"]').forEach((el) => {
      remote += 1
      el.remove()
    })
  }
  const quote = foldPoint(doc, QUOTE_SELECTOR)
  const mark = foldPoint(doc, SIGNATURE_SELECTOR)
  const start = quote && mark ? (quote.compareDocumentPosition(mark) & Node.DOCUMENT_POSITION_PRECEDING ? mark : quote) : quote ?? mark
  let foldable = false
  let folded = 0
  if (start) {
    const range = doc.createRange()
    range.setStartBefore(start)
    range.setEndAfter(doc.body.lastChild ?? start)
    const fold = doc.createElement('div')
    fold.appendChild(range.extractContents())
    foldable = !!doc.body.textContent?.trim()
    if (foldable) fold.className = 'cn-fold'
    if (foldable) folded = fold.querySelectorAll('img[data-blocked]').length
    doc.body.appendChild(fold)
  }
  doc.querySelectorAll('table').forEach((table) => {
    if (table.parentElement?.closest('table')) return
    const scroll = doc.createElement('div')
    scroll.className = 'cn-scroll'
    table.replaceWith(scroll)
    scroll.appendChild(table)
  })
  const head = [...doc.head.querySelectorAll('style, link')].map((el) => el.outerHTML).join('')
  const bodyAttrs = ['bgcolor', 'text', 'link', 'style'].map((name) => doc.body.getAttribute(name)).some(Boolean)
  const rendered = bodyAttrs ? `<div class="cn-body" style="${escapeHtml(bodyStyle(doc.body))}">${doc.body.innerHTML}</div>` : doc.body.innerHTML
  return { html: head + rendered, remote, folded, designed, foldable }
}

function bodyStyle(body: HTMLElement): string {
  const parts = [body.getAttribute('style') ?? '']
  const bg = body.getAttribute('bgcolor')
  const text = body.getAttribute('text')
  if (bg) parts.push(`background-color:${bg}`)
  if (text) parts.push(`color:${text}`)
  return parts.filter(Boolean).join(';')
}

export interface FrameOptions {
  remote: boolean
  dark: boolean
  ink: string
  accent: string
  quote: boolean
}

export function mailDocument(mail: PreparedMail, options: FrameOptions): string {
  const extra = options.remote ? ' https: http:' : ''
  const csp = [`default-src 'none'`, `img-src data: blob:${extra}`, `style-src 'unsafe-inline'${options.remote ? ' https:' : ''}`, `font-src 'self' data:${options.remote ? ' https:' : ''}`, `form-action 'none'`, `base-uri 'none'`].join('; ')
  const inset = options.dark && mail.designed
  const ink = inset ? '#1d1a17' : options.ink
  const css = [
    `@font-face{font-family:'Schibsted Grotesk';font-weight:400 900;src:url('/fonts/schibsted-grotesk-latin-wght-normal.woff2') format('woff2')}`,
    `html,body{margin:0;padding:0;background:transparent;overflow-y:hidden}body{display:flow-root}`,
    `body{font:15px/1.6 'Schibsted Grotesk',system-ui,sans-serif;color:${ink};overflow-wrap:anywhere;-webkit-text-size-adjust:none${mail.designed ? '' : ';max-width:68ch'}}`,
    mail.designed ? '' : `body :not(pre,code,kbd,samp,tt){font-family:inherit!important}`,
    `a{color:${inset ? '#2f4fd8' : options.accent}}`,
    `img{max-width:100%;height:auto}`,
    `img[data-blocked]{background:rgba(127,127,127,.12)}`,
    `.cn-scroll{max-width:100%;overflow-x:auto}`,
    `pre{white-space:pre-wrap}`,
    `blockquote{margin:.5em 0 .5em .5ex;padding-left:1ex;border-left:2px solid rgba(127,127,127,.35)}`,
    `.cn-fold{display:none}`,
    `html.cn-quote .cn-fold{display:block}`,
    `html.cn-invert img,html.cn-invert video,html.cn-invert picture,html.cn-invert canvas,html.cn-invert [style*="background-image"]{filter:invert(1) hue-rotate(180deg)}`
  ].join('')
  return `<!doctype html><html class="${options.quote ? 'cn-quote' : ''}"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${csp}"><meta name="color-scheme" content="light"><style>${css}</style></head><body>${mail.html}</body></html>`
}

export function colorChannels(css: string): { r: number; g: number; b: number; a: number } | null {
  const parts = css.match(/rgba?\(([^)]+)\)/i)?.[1]?.split(/[\s,/]+/).filter(Boolean).map(Number)
  if (!parts || parts.length < 3 || parts.slice(0, 3).some((n) => !Number.isFinite(n))) return null
  return { r: parts[0], g: parts[1], b: parts[2], a: parts.length > 3 && Number.isFinite(parts[3]) ? parts[3] : 1 }
}

export function lightSurface(backgrounds: string[], threshold = 0.72): boolean {
  for (const css of backgrounds) {
    const color = colorChannels(css)
    if (!color || color.a < 0.5) continue
    return (0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b) / 255 > threshold
  }
  return true
}

export function mailBackgrounds(doc: Document): string[] {
  const body = doc.body
  if (!body) return []
  const width = body.getBoundingClientRect().width || 1
  const wide = [...body.querySelectorAll<HTMLElement>('body > *, table, td, div, center')]
    .filter((el) => el.getBoundingClientRect().width >= width * 0.8 && el.getBoundingClientRect().height >= 40)
    .slice(0, 12)
  const view = doc.defaultView
  return [...wide, body, doc.documentElement].map((el) => (view ? view.getComputedStyle(el).backgroundColor : ''))
}
