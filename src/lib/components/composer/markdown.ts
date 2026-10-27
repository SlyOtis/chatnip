import { Lexer, type Token, type Tokens } from 'marked'
import { codeBlock, escapeHtml } from '$lib/content'
import { trimUrl } from '$lib/links'
import { customHtml, findShortcodes, getCustom, getUnicode } from '$lib/emoji'

export type Mark = 'bold' | 'italic' | 'underline' | 'strike' | 'code' | 'link' | 'image'
export type BlockType = 'paragraph' | 'heading' | 'quote' | 'code' | 'bullet' | 'ordered' | 'task' | 'table' | 'rule'

export interface Run {
  text: string
  marks: Mark[]
  href?: string
}

export interface Block {
  type: BlockType
  runs: Run[]
  lang?: string
  level?: number
  depth?: number
  checked?: boolean
}

export type Doc = Block[]

export interface Edit {
  doc: Doc
  pos: number
}

const MARKS: Mark[] = ['bold', 'italic', 'underline', 'strike', 'code', 'link', 'image']
const DELIMITERS: Record<Exclude<Mark, 'link' | 'image'>, [string, string]> = { bold: ['**', '**'], italic: ['_', '_'], underline: ['<u>', '</u>'], strike: ['~', '~'], code: ['`', '`'] }
const LISTS = new Set<BlockType>(['bullet', 'ordered', 'task'])
const RAW = new Set<BlockType>(['code', 'table'])
const URL_AT = /^https?:\/\/[^\s<>"')\]]+/
const URL_END = /https?:\/\/[^\s<>"')\]]+$/
const SAFE_HREF = /^(https?:\/\/|mailto:|mention:|channel:)/i

export function isReference(href: string | undefined): boolean {
  return !!href && (href.startsWith('mention:') || href.startsWith('channel:'))
}
const TABLE_DELIMITER = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/
const RULE_LINE = /^\s{0,3}([-*_])(?:\s*\1){2,}\s*$/
const MAX_DEPTH = 5

export const TABLE_TEMPLATE = '|  |  |\n| --- | --- |\n|  |  |'

export function isList(block: Block): boolean {
  return LISTS.has(block.type)
}

export function isRaw(block: Block): boolean {
  return RAW.has(block.type)
}

export type EnterAction = 'send' | 'send-done' | 'edit' | 'compose'

export interface EnterKey {
  shiftKey: boolean
  metaKey: boolean
  ctrlKey: boolean
  isComposing?: boolean
  keyCode?: number
}

export function enterAction(block: Block | undefined, key: EnterKey, sendsOnEnter: boolean, composing = false): EnterAction {
  if (composing || key.isComposing || key.keyCode === 229) return 'compose'
  if (key.metaKey || key.ctrlKey) return key.shiftKey ? 'send-done' : 'send'
  if (key.shiftKey || !sendsOnEnter) return 'edit'
  if (block && (isRaw(block) || isList(block))) return 'edit'
  if (block?.type === 'paragraph' && opensBlock(blockText(block))) return 'edit'
  return 'send'
}

function opensBlock(text: string): boolean {
  return /^```\w*$/.test(text) || RULE_LINE.test(text) || (/^\|.*\|$/.test(text.trim()) && tableCells(text).length >= 2)
}

export function emptyDoc(): Doc {
  return [{ type: 'paragraph', runs: [] }]
}

export function run(text: string, marks: Mark[] = [], href?: string): Run {
  const set = new Set(marks)
  if (href) set.add('link')
  else {
    set.delete('link')
    set.delete('image')
  }
  return href ? { text, marks: MARKS.filter((m) => set.has(m)), href } : { text, marks: MARKS.filter((m) => set.has(m)) }
}

export function blockOf(type: BlockType, runs: Run[], attrs: Partial<Block> = {}): Block {
  const block: Block = { type, runs: type === 'rule' ? [] : runs }
  if (type === 'code' && attrs.lang) block.lang = attrs.lang
  if (type === 'heading') block.level = Math.min(6, Math.max(1, attrs.level ?? 1))
  if (LISTS.has(type) && attrs.depth) block.depth = Math.min(MAX_DEPTH, attrs.depth)
  if (type === 'task') block.checked = !!attrs.checked
  return block
}

function sameMarks(a: Run, b: Run): boolean {
  return a.href === b.href && a.marks.length === b.marks.length && a.marks.every((m, i) => m === b.marks[i])
}

export function normalizeRuns(runs: Run[]): Run[] {
  const out: Run[] = []
  for (const r of runs) {
    if (!r.text) continue
    const last = out[out.length - 1]
    if (last && sameMarks(last, r) && !r.marks.includes('image')) last.text += r.text
    else out.push(run(r.text, r.marks, r.href))
  }
  return out
}

function cloneBlock(b: Block): Block {
  return { ...b, runs: b.runs.map((r) => run(r.text, r.marks, r.href)) }
}

export function cloneDoc(doc: Doc): Doc {
  return doc.map(cloneBlock)
}

export function blockText(b: Block): string {
  return b.runs.map((r) => r.text).join('')
}

export function docText(doc: Doc): string {
  return doc.map(blockText).join('\n')
}

export function isEmpty(doc: Doc): boolean {
  return doc.every((b) => b.type !== 'rule' && blockText(b).trim() === '')
}

export function locate(doc: Doc, pos: number): { index: number; offset: number } {
  let rest = Math.max(0, pos)
  for (let i = 0; i < doc.length; i++) {
    const len = blockText(doc[i]).length
    if (rest <= len) return { index: i, offset: rest }
    rest -= len + 1
  }
  const last = doc.length - 1
  return { index: last, offset: blockText(doc[last]).length }
}

export function blockStart(doc: Doc, index: number): number {
  let pos = 0
  for (let i = 0; i < index; i++) pos += blockText(doc[i]).length + 1
  return pos
}

export function sliceRuns(runs: Run[], from: number, to = Infinity): Run[] {
  const out: Run[] = []
  let at = 0
  for (const r of runs) {
    const end = at + r.text.length
    const a = Math.max(from, at)
    const b = Math.min(to, end)
    if (b > a) out.push(run(r.text.slice(a - at, b - at), r.marks, r.href))
    at = end
  }
  return out
}

function mapRange(runs: Run[], from: number, to: number, fn: (r: Run) => Run): Run[] {
  return normalizeRuns([...sliceRuns(runs, 0, from), ...sliceRuns(runs, from, to).map(fn), ...sliceRuns(runs, to)])
}

export function marksAt(doc: Doc, pos: number): Mark[] {
  const { index, offset } = locate(doc, pos)
  const block = doc[index]
  if (isRaw(block) || block.type === 'rule' || offset === 0) return []
  const r = sliceRuns(block.runs, offset - 1, offset)[0]
  return r ? r.marks.filter((m) => m !== 'link' && m !== 'image') : []
}

export function insertText(doc: Doc, pos: number, text: string, marks: Mark[] = [], href?: string): Edit {
  const { index, offset } = locate(doc, pos)
  const base = doc[index].type === 'rule' ? setBlockType(doc, index, 'paragraph') : doc
  const block = base[index]
  if (!isRaw(block) && text.includes('\n')) {
    const lines = text.split('\n').map((line) => blockOf('paragraph', linkify([run(line, marks, href)])))
    return insertDoc(base, pos, lines)
  }
  const next = cloneDoc(base)
  const inserted = isRaw(block) ? run(text) : run(text, marks, href)
  next[index].runs = normalizeRuns([...sliceRuns(block.runs, 0, offset), inserted, ...sliceRuns(block.runs, offset)])
  return { doc: next, pos: pos + text.length }
}

export function deleteRange(doc: Doc, from: number, to: number): Edit {
  if (to < from) [from, to] = [to, from]
  if (from === to) return { doc, pos: from }
  const a = locate(doc, from)
  const b = locate(doc, to)
  const next = cloneDoc(doc)
  const first = next[a.index]
  first.runs = normalizeRuns([...sliceRuns(doc[a.index].runs, 0, a.offset), ...sliceRuns(doc[b.index].runs, b.offset)])
  next.splice(a.index + 1, b.index - a.index)
  return { doc: next, pos: from }
}

export function insertDoc(doc: Doc, pos: number, fragment: Doc): Edit {
  const frag = fragment.length ? fragment : emptyDoc()
  const { index, offset } = locate(doc, pos)
  const block = doc[index]
  if (isRaw(block)) return insertText(doc, pos, docText(frag))
  const left = sliceRuns(block.runs, 0, offset)
  const right = sliceRuns(block.runs, offset)
  const blocks = frag.map(cloneBlock)
  if (!isRaw(blocks[0]) && blocks[0].type !== 'rule') blocks[0] = left.length || block.type !== 'paragraph' ? { ...cloneBlock(block), runs: normalizeRuns([...left, ...blocks[0].runs]) } : { ...blocks[0], runs: normalizeRuns([...left, ...blocks[0].runs]) }
  else if (left.length) blocks.unshift({ ...cloneBlock(block), runs: left })
  const last = blocks[blocks.length - 1]
  let caret = 0
  if (!isRaw(last) && last.type !== 'rule') {
    caret = blockText(last).length
    last.runs = normalizeRuns([...last.runs, ...right])
  } else blocks.push(right.length ? { ...cloneBlock(block), runs: right } : blockOf('paragraph', []))
  const next = [...cloneDoc(doc.slice(0, index)), ...blocks, ...cloneDoc(doc.slice(index + 1))]
  return { doc: next, pos: blockStart(next, index + blocks.length - 1) + caret }
}

export function setBlockType(doc: Doc, index: number, type: BlockType, attrs: Partial<Block> = {}): Doc {
  const next = cloneDoc(doc)
  const block = next[index]
  const runs = RAW.has(type) || isRaw(block) ? normalizeRuns([run(blockText(block))]) : block.runs
  const keep: Partial<Block> = isList(block) && LISTS.has(type) ? { depth: block.depth, checked: block.checked } : {}
  next[index] = blockOf(type, runs, { ...keep, ...attrs })
  return next
}

function rawLines(doc: Doc, index: number): Doc {
  const lines = blockText(doc[index]).split('\n').map((line) => blockOf('paragraph', normalizeRuns([run(line)])))
  const next = cloneDoc(doc)
  next.splice(index, 1, ...lines)
  return next
}

export function splitBlock(doc: Doc, pos: number): Edit {
  const { index, offset } = locate(doc, pos)
  const block = doc[index]
  if (isRaw(block)) return insertText(doc, pos, '\n')
  const text = blockText(block)
  if (block.type !== 'paragraph' && text === '') {
    if (isList(block) && (block.depth ?? 0) > 0) return { doc: setBlockType(doc, index, block.type, { depth: (block.depth ?? 0) - 1 }), pos }
    return { doc: setBlockType(doc, index, 'paragraph'), pos }
  }
  const next = cloneDoc(doc)
  if (block.type === 'heading' && offset === 0) {
    next.splice(index, 0, blockOf('paragraph', []))
    return { doc: next, pos: pos + 1 }
  }
  const tail = block.type === 'heading' || block.type === 'rule' ? 'paragraph' : block.type
  next.splice(index, 1, { ...cloneBlock(block), runs: sliceRuns(block.runs, 0, offset) }, blockOf(tail, sliceRuns(block.runs, offset), { ...block, checked: false }))
  return { doc: next, pos: pos + 1 }
}

function tableCells(line: string): string[] {
  return line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((c) => c.trim())
}

function currentLine(text: string, offset: number): { start: number; end: number; line: string } {
  const start = text.lastIndexOf('\n', offset - 1) + 1
  const found = text.indexOf('\n', offset)
  const end = found < 0 ? text.length : found
  return { start, end, line: text.slice(start, end) }
}

function leaveRaw(doc: Doc, index: number, start: number, end: number): Edit {
  const block = doc[index]
  const text = blockText(block)
  const before = text.slice(0, Math.max(0, start - 1))
  const after = end >= text.length ? '' : text.slice(end + 1)
  const next = cloneDoc(doc)
  next.splice(index, 1, { ...cloneBlock(block), runs: normalizeRuns([run(before)]) }, blockOf('paragraph', normalizeRuns([run(after)])))
  return { doc: next, pos: blockStart(next, index + 1) }
}

export function enter(doc: Doc, pos: number): Edit {
  const { index, offset } = locate(doc, pos)
  const block = doc[index]
  const text = blockText(block)
  if (block.type === 'code') {
    const { start, end, line } = currentLine(text, offset)
    if (line.trim() === '```') return leaveRaw(doc, index, start, end)
    return insertText(doc, pos, '\n')
  }
  if (block.type === 'table') {
    const { start, end, line } = currentLine(text, offset)
    if (line.trim() === '' || line.trim() === '|') return leaveRaw(doc, index, start, end)
    return insertText(doc, pos, '\n| ')
  }
  if (block.type === 'rule') {
    const next = cloneDoc(doc)
    next.splice(index + 1, 0, blockOf('paragraph', []))
    return { doc: next, pos: pos + 1 }
  }
  if (block.type === 'paragraph' && offset === text.length) {
    const fence = /^```(\w*)$/.exec(text)
    if (fence) return { doc: setBlockType(cloneDoc(doc).map((b, i) => (i === index ? { ...b, runs: [] } : b)), index, 'code', { lang: fence[1] || undefined }), pos: pos - text.length }
    if (RULE_LINE.test(text)) {
      const next = cloneDoc(doc)
      next.splice(index, 1, blockOf('rule', []), blockOf('paragraph', []))
      return { doc: next, pos: blockStart(next, index + 1) }
    }
    const cells = tableCells(text)
    if (/^\|.*\|$/.test(text.trim()) && cells.length >= 2) {
      const table = `${text.trim()}\n| ${cells.map(() => '---').join(' | ')} |\n| `
      const next = cloneDoc(doc)
      next.splice(index, 1, blockOf('table', [run(table)]))
      return { doc: next, pos: blockStart(next, index) + table.length }
    }
  }
  return splitBlock(doc, pos)
}

export function exitCode(doc: Doc, pos: number): Edit | null {
  const { index, offset } = locate(doc, pos)
  const block = doc[index]
  if (!isRaw(block) || offset !== blockText(block).length || index !== doc.length - 1) return null
  const next = cloneDoc(doc)
  next.push(blockOf('paragraph', []))
  return { doc: next, pos: pos + 1 }
}

export function backspaceAtStart(doc: Doc, pos: number): Edit | null {
  const { index, offset } = locate(doc, pos)
  if (offset !== 0) return null
  const block = doc[index]
  if (isRaw(block)) return { doc: rawLines(doc, index), pos }
  if (isList(block) && (block.depth ?? 0) > 0) return { doc: setBlockType(doc, index, block.type, { depth: (block.depth ?? 0) - 1 }), pos }
  if (block.type !== 'paragraph') return { doc: setBlockType(doc, index, 'paragraph'), pos }
  if (index === 0) return null
  const prev = doc[index - 1]
  const next = cloneDoc(doc)
  if (prev.type === 'rule') {
    next.splice(index - 1, 1)
    return { doc: next, pos: pos - 1 }
  }
  const merged = isRaw(prev) ? [run(blockText(block))] : block.runs
  next.splice(index - 1, 2, { ...cloneBlock(prev), runs: normalizeRuns([...prev.runs, ...merged]) })
  return { doc: next, pos: pos - 1 }
}

export function indent(doc: Doc, from: number, to: number, delta: number): Doc | null {
  const a = locate(doc, from).index
  const b = locate(doc, to).index
  if (!doc.slice(a, b + 1).some(isList)) return null
  const next = cloneDoc(doc)
  for (let i = a; i <= b; i++) {
    const block = next[i]
    if (!isList(block)) continue
    const ceiling = i > 0 && isList(next[i - 1]) ? (next[i - 1].depth ?? 0) + 1 : 0
    next[i] = blockOf(block.type, block.runs, { ...block, depth: Math.max(0, Math.min(ceiling, MAX_DEPTH, (block.depth ?? 0) + delta)) })
  }
  return next
}

export function toggleTask(doc: Doc, index: number): Doc {
  const block = doc[index]
  if (!block || block.type !== 'task') return doc
  const next = cloneDoc(doc)
  next[index] = { ...next[index], checked: !block.checked }
  return next
}

export function setBlocks(doc: Doc, from: number, to: number, type: BlockType, attrs: Partial<Block> = {}): Doc {
  const a = locate(doc, from).index
  const b = locate(doc, to).index
  const range = doc.slice(a, b + 1)
  const same = range.every((blk) => blk.type === type && (type !== 'heading' || blk.level === (attrs.level ?? 1)))
  if (RAW.has(type)) {
    if (same) {
      let next = doc
      for (let i = b; i >= a; i--) next = rawLines(next, i)
      return next
    }
    const next = cloneDoc(doc)
    next.splice(a, b - a + 1, blockOf(type, normalizeRuns([run(range.map(blockText).join('\n'))]), attrs))
    return next
  }
  let next = doc
  for (let i = a; i <= b; i++) {
    if (isRaw(next[i])) continue
    next = setBlockType(next, i, same ? 'paragraph' : type, attrs)
  }
  return next
}

export function insertBlock(doc: Doc, pos: number, block: Block, caret = 0): Edit {
  const { index } = locate(doc, pos)
  const current = doc[index]
  const replace = current.type === 'paragraph' && blockText(current) === ''
  const next = cloneDoc(doc)
  const at = replace ? index : index + 1
  next.splice(at, replace ? 1 : 0, cloneBlock(block))
  if (block.type === 'rule' && at === next.length - 1) next.push(blockOf('paragraph', []))
  return { doc: next, pos: blockStart(next, block.type === 'rule' ? at + 1 : at) + caret }
}

function rangeHas(doc: Doc, from: number, to: number, mark: Mark): boolean {
  let any = false
  for (let i = 0; i < doc.length; i++) {
    const start = blockStart(doc, i)
    const block = doc[i]
    if (isRaw(block)) continue
    const a = Math.max(0, from - start)
    const b = Math.min(blockText(block).length, to - start)
    if (b <= a) continue
    for (const r of sliceRuns(block.runs, a, b)) {
      if (!r.marks.includes(mark)) return false
      any = true
    }
  }
  return any
}

export function setMark(doc: Doc, from: number, to: number, mark: Mark, on: boolean, href?: string): Doc {
  return doc.map((block, i) => {
    const start = blockStart(doc, i)
    const a = Math.max(0, from - start)
    const b = Math.min(blockText(block).length, to - start)
    if (isRaw(block) || b <= a) return cloneBlock(block)
    const runs = mapRange(block.runs, a, b, (r) => {
      if (mark === 'link') return run(r.text, r.marks, on ? href : undefined)
      const marks = on ? [...r.marks, mark] : r.marks.filter((m) => m !== mark)
      return run(r.text, marks, r.href)
    })
    return { ...block, runs }
  })
}

export function toggleMark(doc: Doc, from: number, to: number, mark: Mark): Doc {
  return setMark(doc, from, to, mark, !rangeHas(doc, from, to, mark))
}

export function linkAt(doc: Doc, pos: number): string | undefined {
  const { index, offset } = locate(doc, pos)
  const block = doc[index]
  const r = sliceRuns(block.runs, Math.max(0, offset - 1), offset + 1).find((x) => x.href)
  return r?.href
}

interface InlineRule {
  pattern: RegExp
  mark: Mark
  open: number
  close: number
}

const INLINE_RULES: InlineRule[] = [
  { pattern: /(`([^`]+)`)$/, mark: 'code', open: 1, close: 1 },
  { pattern: /(?:^|[^\w*])(\*\*([^*\s](?:[^*]*[^*\s])?)\*\*)$/, mark: 'bold', open: 2, close: 2 },
  { pattern: /(?:^|[^~])(~~([^~\s](?:[^~]*[^~\s])?)~~)$/, mark: 'strike', open: 2, close: 2 },
  { pattern: /(?:^|[^\w*])(\*([^*\s](?:[^*]*[^*\s])?)\*)$/, mark: 'italic', open: 1, close: 1 },
  { pattern: /(?:^|[^\w_])(_([^_\s](?:[^_]*[^_\s])?)_)$/, mark: 'italic', open: 1, close: 1 },
  { pattern: /(?:^|[^\w~])(~([^~\s](?:[^~]*[^~\s])?)~)$/, mark: 'strike', open: 1, close: 1 }
]

const BLOCK_RULES: { pattern: RegExp; type: BlockType; attrs?: (m: RegExpExecArray) => Partial<Block> }[] = [
  { pattern: /^(#{1,6}) $/, type: 'heading', attrs: (m) => ({ level: m[1].length }) },
  { pattern: /^> $/, type: 'quote' },
  { pattern: /^[-*+] \[([ xX]?)\] $/, type: 'task', attrs: (m) => ({ checked: m[1].trim() !== '' }) },
  { pattern: /^\[([ xX]?)\] $/, type: 'task', attrs: (m) => ({ checked: m[1].trim() !== '' }) },
  { pattern: /^[-*+] $/, type: 'bullet' },
  { pattern: /^\d+[.)] $/, type: 'ordered' }
]

export interface RuleResult extends Edit {
  fired: boolean
}

function imageName(src: string): string {
  return decodeURIComponent(src.split(/[?#]/)[0].split('/').filter(Boolean).pop() ?? src)
}

export function applyRules(doc: Doc, pos: number): RuleResult {
  const { index, offset } = locate(doc, pos)
  const block = doc[index]
  if (isRaw(block) || block.type === 'rule') return { doc, pos, fired: false }
  const start = blockStart(doc, index)
  const before = blockText(block).slice(0, offset)

  const rules = block.type === 'paragraph' ? BLOCK_RULES : block.type === 'bullet' ? BLOCK_RULES.slice(3, 4) : []
  for (const rule of rules) {
    const m = rule.pattern.exec(before)
    if (m) {
      const stripped = deleteRange(doc, start, start + before.length)
      return { doc: setBlockType(stripped.doc, index, rule.type, rule.attrs?.(m)), pos: start, fired: true }
    }
  }

  const image = /(!\[([^\]]*)\]\((https?:\/\/[^\s)]+)\))$/.exec(before)
  if (image) {
    const from = pos - image[1].length
    const alt = image[2] || imageName(image[3])
    const base = deleteRange(doc, from, pos).doc
    const r = insertText(base, from, alt, [], image[3])
    const next = setMark(r.doc, from, from + alt.length, 'image', true)
    return { doc: next, pos: from + alt.length, fired: true }
  }

  const link = /(\[([^\]]+)\]\((https?:\/\/[^\s)]+|mailto:[^\s)]+)\))$/.exec(before)
  if (link) {
    const from = pos - link[1].length
    const label = link[2]
    let next = deleteRange(doc, from + 1 + label.length, pos).doc
    next = deleteRange(next, from, from + 1).doc
    return { doc: setMark(next, from, from + label.length, 'link', true, link[3]), pos: from + label.length, fired: true }
  }

  const auto = /(<((?:https?:\/\/|mailto:)[^\s<>]+)>)$/.exec(before)
  if (auto) {
    const from = pos - auto[1].length
    const label = auto[2].replace(/^mailto:/, '')
    const base = deleteRange(doc, from, pos).doc
    const r = insertText(base, from, label, [], auto[2])
    return { doc: r.doc, pos: r.pos, fired: true }
  }

  const tail = before.slice(-1)
  if (/\s/.test(tail)) {
    const url = URL_END.exec(before.slice(0, -1))
    if (url) {
      const to = pos - 1
      const from = to - url[0].length
      const trimmed = trimUrl(url[0])
      if (!linkAt(doc, from + 1)) return { doc: setMark(doc, from, from + trimmed.length, 'link', true, trimmed), pos, fired: false }
    }
  }

  for (const rule of INLINE_RULES) {
    const m = rule.pattern.exec(before)
    if (!m) continue
    const from = pos - m[1].length
    const inner = sliceRuns(block.runs, from - start, offset)
    if (inner.some((r) => r.marks.includes('code'))) continue
    let next = setMark(doc, from + rule.open, pos - rule.close, rule.mark, true)
    next = deleteRange(next, pos - rule.close, pos).doc
    next = deleteRange(next, from, from + rule.open).doc
    return { doc: next, pos: pos - rule.open - rule.close, fired: true }
  }

  return { doc, pos, fired: false }
}

export function mentionRun(id: string, name: string): Run {
  return run(`@${name}`, [], `mention:${id}`)
}

export function channelRun(id: string, name: string): Run {
  return run(`#${name}`, [], `channel:${id}`)
}

export function linkify(runs: Run[]): Run[] {
  const out: Run[] = []
  for (const r of runs) {
    if (r.href || r.marks.includes('code')) {
      out.push(r)
      continue
    }
    let rest = r.text
    let buf = ''
    while (rest.length) {
      const m = URL_AT.exec(rest)
      if (m && !/\w$/.test(buf)) {
        const url = trimUrl(m[0])
        if (buf) out.push(run(buf, r.marks))
        out.push(run(url, r.marks, url))
        buf = ''
        rest = rest.slice(url.length)
        continue
      }
      buf += rest[0]
      rest = rest.slice(1)
    }
    if (buf) out.push(run(buf, r.marks))
  }
  return normalizeRuns(out)
}

export function textToDoc(text: string): Doc {
  return text.replace(/\r\n?/g, '\n').split('\n').map((line) => blockOf('paragraph', linkify([run(line)])))
}

function tokenRuns(tokens: Token[], marks: Mark[], href: string | undefined, out: Run[]) {
  let current = marks
  for (const token of tokens) {
    switch (token.type) {
      case 'strong':
        tokenRuns((token as Tokens.Strong).tokens, [...current, 'bold'], href, out)
        break
      case 'em':
        tokenRuns((token as Tokens.Em).tokens, [...current, 'italic'], href, out)
        break
      case 'del':
        tokenRuns((token as Tokens.Del).tokens, [...current, 'strike'], href, out)
        break
      case 'codespan':
        out.push(run((token as Tokens.Codespan).text, [...current, 'code'], href))
        break
      case 'link': {
        const link = token as Tokens.Link
        const written = link.raw.startsWith('[') || link.raw.startsWith('<') || link.raw === link.href
        if (href || !written) tokenRuns(link.tokens, current, href, out)
        else if (!SAFE_HREF.test(link.href)) out.push(run(link.raw, current))
        else if (isReference(link.href)) out.push(run(link.text, [], link.href))
        else tokenRuns(link.tokens, current, link.href, out)
        break
      }
      case 'image': {
        const image = token as Tokens.Image
        if (/^https?:\/\//i.test(image.href)) out.push(run(image.text || imageName(image.href), [...current, 'image'], image.href))
        else out.push(run(image.raw, current, href))
        break
      }
      case 'html':
        if (/^<u>$/i.test(token.raw)) current = [...marks, 'underline']
        else if (/^<\/u>$/i.test(token.raw)) current = marks
        else out.push(run(token.raw, current, href))
        break
      case 'br':
        break
      case 'text': {
        const text = token as Tokens.Text
        if (text.tokens?.length) tokenRuns(text.tokens, current, href, out)
        else out.push(run(text.text, current, href))
        break
      }
      case 'escape':
        out.push(run((token as Tokens.Escape).text, current, href))
        break
      default:
        out.push(run(token.raw, current, href))
    }
  }
}

export function parseInline(s: string, marks: Mark[] = [], href?: string): Run[] {
  if (!s) return []
  const out: Run[] = []
  tokenRuns(Lexer.lexInline(s, { gfm: true }), marks, href, out)
  return normalizeRuns(out)
}

export function fromMarkdown(text: string): Doc {
  const lines = text.replace(/\r\n?/g, '\n').split('\n')
  const doc: Doc = []
  let indents: number[] = []
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const fence = /^(```|~~~)(\w*)\s*$/.exec(line)
    if (fence) {
      const body: string[] = []
      i++
      while (i < lines.length && !new RegExp(`^${fence[1]}\\s*$`).test(lines[i])) body.push(lines[i++])
      doc.push(blockOf('code', normalizeRuns([run(body.join('\n'))]), { lang: fence[2] || undefined }))
      indents = []
      continue
    }
    if (line.includes('|') && i + 1 < lines.length && lines[i + 1].includes('|') && TABLE_DELIMITER.test(lines[i + 1])) {
      const rows = [line, lines[i + 1]]
      i += 2
      while (i < lines.length && lines[i].includes('|') && lines[i].trim() !== '') rows.push(lines[i++])
      i--
      doc.push(blockOf('table', [run(rows.join('\n'))]))
      indents = []
      continue
    }
    if (RULE_LINE.test(line)) {
      doc.push(blockOf('rule', []))
      indents = []
      continue
    }
    const heading = /^\s{0,3}(#{1,6})\s+(.*?)(?:\s+#+)?\s*$/.exec(line)
    if (heading) {
      doc.push(blockOf('heading', parseInline(heading[2]), { level: heading[1].length }))
      indents = []
      continue
    }
    const item = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/.exec(line)
    if (item) {
      const width = item[1].replace(/\t/g, '    ').length
      if (!doc.length || !isList(doc[doc.length - 1]) || !indents.length) indents = [width]
      else {
        while (indents.length > 1 && width < indents[indents.length - 1]) indents.pop()
        if (width > indents[indents.length - 1]) indents.push(width)
      }
      const depth = indents.length - 1
      const task = /^[-*+]$/.test(item[2]) ? /^\[([ xX])\](?:\s+(.*))?$/.exec(item[3]) : null
      if (task) doc.push(blockOf('task', parseInline(task[2] ?? ''), { depth, checked: task[1] !== ' ' }))
      else doc.push(blockOf(/^\d/.test(item[2]) ? 'ordered' : 'bullet', parseInline(item[3]), { depth }))
      continue
    }
    indents = []
    const quote = /^> ?(.*)$/.exec(line)
    if (quote) {
      doc.push(blockOf('quote', parseInline(quote[1])))
      continue
    }
    doc.push(blockOf('paragraph', parseInline(line)))
  }
  return doc.length ? doc : emptyDoc()
}

const INLINE_SPECIAL = /[\\`*_~[\]<]/g

function escapeText(text: string): string {
  return text.replace(INLINE_SPECIAL, '\\$&')
}

function inlineMarkdown(runs: Run[], escape = false): string {
  let out = ''
  let i = 0
  while (i < runs.length) {
    const r = runs[i]
    if (r.href && r.marks.includes('image')) {
      out += `![${escape ? escapeText(r.text) : r.text}](${r.href})`
      i++
      continue
    }
    if (r.href) {
      const group: Run[] = []
      while (i < runs.length && runs[i].href === r.href && !runs[i].marks.includes('image')) group.push(runs[i++])
      const label = group.map((g) => g.text).join('')
      const plain = group.every((g) => g.marks.length === 1)
      if (isReference(r.href)) out += `[${escape ? escapeText(label) : label}](${r.href})`
      else out += plain && label === r.href ? label : `[${inlineMarkdown(group.map((g) => run(g.text, g.marks)), escape)}](${r.href})`
      continue
    }
    const group: Run[] = []
    while (i < runs.length && !runs[i].href) group.push(runs[i++])
    out += markedMarkdown(group, escape)
  }
  return out
}

function markedMarkdown(runs: Run[], escape = false): string {
  const order: Exclude<Mark, 'link' | 'image'>[] = ['bold', 'italic', 'underline', 'strike', 'code']
  let stack: Exclude<Mark, 'link' | 'image'>[] = []
  let out = ''
  for (const r of runs) {
    const want = order.filter((m) => r.marks.includes(m))
    let keep = 0
    while (keep < stack.length && keep < want.length && stack[keep] === want[keep]) keep++
    for (let k = stack.length - 1; k >= keep; k--) out += DELIMITERS[stack[k]][1]
    stack = stack.slice(0, keep)
    for (const m of want.slice(keep)) {
      out += DELIMITERS[m][0]
      stack.push(m)
    }
    out += escape && !r.marks.includes('code') ? escapeText(r.text) : r.text
  }
  for (let k = stack.length - 1; k >= 0; k--) out += DELIMITERS[stack[k]][1]
  return out
}

function listNumbers(doc: Doc): number[] {
  const counters: number[] = []
  return doc.map((block) => {
    if (!isList(block)) {
      counters.length = 0
      return 0
    }
    const depth = block.depth ?? 0
    while (counters.length < depth + 1) counters.push(0)
    counters.length = depth + 1
    counters[depth] = block.type === 'ordered' ? counters[depth] + 1 : 0
    return counters[depth]
  })
}

function blockPrefix(block: Block, n: number): string {
  const lead = '  '.repeat(block.depth ?? 0)
  if (block.type === 'code') return '```' + (block.lang ?? '') + '\n'
  if (block.type === 'heading') return `${'#'.repeat(block.level ?? 1)} `
  if (block.type === 'quote') return '> '
  if (block.type === 'bullet') return `${lead}- `
  if (block.type === 'task') return `${lead}- [${block.checked ? 'x' : ' '}] `
  if (block.type === 'ordered') return `${lead}${n}. `
  return ''
}

function blockBody(block: Block, escape: boolean): string {
  if (block.type === 'code') return blockText(block) + '\n```'
  if (block.type === 'table') return blockText(block)
  if (block.type === 'rule') return '---'
  const body = inlineMarkdown(block.runs, escape)
  if (!escape || block.type !== 'paragraph') return body
  if (/^\s*\d+[.)](\s|$)/.test(body)) return body.replace(/^(\s*\d+)([.)])/, '$1\\$2')
  return /^\s*(#|>|[-+]|\||---)/.test(body) ? body.replace(/^(\s*)/, '$1\\') : body
}

function sameBlock(a: Block | undefined, b: Block): boolean {
  return !!a && a.type === b.type && (a.level ?? 0) === (b.level ?? 0) && !!a.checked === !!b.checked && JSON.stringify(a.runs) === JSON.stringify(normalizeRuns(b.runs))
}

function blockSource(block: Block, n: number): string {
  const plain = blockPrefix(block, n) + blockBody(block, false)
  if (isRaw(block) || block.type === 'rule') return plain
  const parsed = fromMarkdown(plain)
  return parsed.length === 1 && sameBlock(parsed[0], block) ? plain : blockPrefix(block, n) + blockBody(block, true)
}

export interface SourceMap {
  text: string
  map: number[]
}

export function markdownMap(doc: Doc): SourceMap {
  const numbers = listNumbers(doc)
  const lines = doc.map((block, i) => blockSource(block, numbers[i]))
  let count = lines.length
  while (count && lines[count - 1].trim() === '') count--
  const text = lines.slice(0, count).join('\n')
  const map: number[] = []
  let base = 0
  doc.forEach((block, i) => {
    const line = lines[i]
    const visible = blockText(block)
    let at = block.type === 'rule' ? line.length : blockPrefix(block, numbers[i]).length
    for (const ch of visible) {
      const found = line.indexOf(ch, at)
      map.push(Math.min(base + (found < 0 ? at : found), text.length))
      at = found < 0 ? at : found + 1
    }
    map.push(Math.min(base + at, text.length))
    base += line.length + 1
  })
  return { text, map }
}

export function visibleOffset(source: SourceMap, offset: number): number {
  let best = 0
  for (let i = 0; i < source.map.length; i++) if (source.map[i] <= offset) best = i
  return best
}

export function sourceOffset(source: SourceMap, pos: number): number {
  return source.map[Math.max(0, Math.min(pos, source.map.length - 1))] ?? source.text.length
}

export function toMarkdown(doc: Doc): string {
  return markdownMap(doc).text
}

export function hasMarkdown(doc: Doc): boolean {
  return doc.some((block) => block.type !== 'paragraph' || block.runs.some((r) => r.marks.some((m) => m !== 'link') || (r.href && r.href !== r.text && !isReference(r.href))))
}

export function looksLikeMarkdown(text: string): boolean {
  return hasMarkdown(fromMarkdown(text))
}

const TAGS: Record<Exclude<Mark, 'link' | 'image'>, string> = { bold: 'strong', italic: 'em', underline: 'u', strike: 's', code: 'code' }

function editorText(text: string): string {
  const hits = findShortcodes(text)
  if (!hits.length) return escapeHtml(text)
  let out = ''
  let at = 0
  for (const hit of hits) {
    const unicode = getUnicode(hit.code)
    const custom = unicode ? null : getCustom(hit.code)
    if (!unicode && !custom) continue
    out += escapeHtml(text.slice(at, hit.start)) + `<span data-emoji="${escapeHtml(hit.code)}" contenteditable="false">${unicode ?? customHtml(custom!, hit.code)}</span>`
    at = hit.end
  }
  return out + escapeHtml(text.slice(at))
}

function inlineHtml(runs: Run[], editor: boolean): string {
  return runs
    .map((r) => {
      if (r.href && r.marks.includes('image')) {
        if (editor) return `<a href="${escapeHtml(r.href)}" data-image="">${escapeHtml(r.text)}</a>`
        return `<img src="${escapeHtml(r.href)}" alt="${escapeHtml(r.text)}" loading="lazy">`
      }
      let html = editor && !r.href && !r.marks.includes('code') ? editorText(r.text) : escapeHtml(r.text)
      for (const m of ['code', 'strike', 'underline', 'italic', 'bold'] as const) if (r.marks.includes(m)) html = `<${TAGS[m]}>${html}</${TAGS[m]}>`
      if (!r.href) return html
      if (isReference(r.href)) return editor ? `<a href="${escapeHtml(r.href)}" data-mention="">${html}</a>` : `<span class="msg-mention">${html}</span>`
      if (editor) return `<a href="${escapeHtml(r.href)}">${html}</a>`
      return SAFE_HREF.test(r.href) ? `<a href="${escapeHtml(r.href)}" target="_blank" rel="noopener noreferrer" class="msg-link">${html}</a>` : html
    })
    .join('')
}

function tableHtml(text: string): string {
  const rows = text.split('\n')
  const aligns = tableCells(rows[1] ?? '').map((cell) => (cell.startsWith(':') && cell.endsWith(':') ? 'center' : cell.endsWith(':') ? 'right' : cell.startsWith(':') ? 'left' : ''))
  const cell = (tag: string, value: string, k: number) => `<${tag}${aligns[k] ? ` style="text-align:${aligns[k]}"` : ''}>${inlineHtml(parseInline(value), false)}</${tag}>`
  const head = tableCells(rows[0]).map((value, k) => cell('th', value, k)).join('')
  const body = rows.slice(2).map((row) => {
    const values = tableCells(row)
    return `<tr>${tableCells(rows[0]).map((_, k) => cell('td', values[k] ?? '', k)).join('')}</tr>`
  })
  return `<table><thead><tr>${head}</tr></thead><tbody>${body.join('')}</tbody></table>`
}

function listHtml(doc: Doc, from: number, to: number): string {
  let out = ''
  let i = from
  while (i < to) {
    const depth = doc[i].depth ?? 0
    const ordered = doc[i].type === 'ordered'
    out += ordered ? '<ol class="msg-list-ordered">' : '<ul class="msg-list">'
    while (i < to && (doc[i].depth ?? 0) === depth && (doc[i].type === 'ordered') === ordered) {
      const block = doc[i]
      const box = block.type === 'task' ? `<input type="checkbox" disabled${block.checked ? ' checked' : ''}> ` : ''
      out += `<li${block.type === 'task' ? ' class="msg-task"' : ''}>${box}${inlineHtml(block.runs, false)}`
      i++
      const start = i
      while (i < to && (doc[i].depth ?? 0) > depth) i++
      if (start < i) out += listHtml(doc, start, i)
      out += '</li>'
    }
    out += ordered ? '</ol>' : '</ul>'
  }
  return out
}

export function toHtml(doc: Doc): string {
  const out: string[] = []
  let i = 0
  while (i < doc.length) {
    const block = doc[i]
    if (isList(block)) {
      const start = i
      while (i < doc.length && isList(doc[i])) i++
      out.push(listHtml(doc, start, i))
      continue
    }
    if (block.type === 'quote') {
      const lines: string[] = []
      while (i < doc.length && doc[i].type === 'quote') lines.push(inlineHtml(doc[i++].runs, false))
      out.push(`<blockquote class="msg-blockquote">${lines.join('<br>')}</blockquote>`)
      continue
    }
    if (block.type === 'paragraph') {
      const lines: string[] = []
      while (i < doc.length && doc[i].type === 'paragraph' && blockText(doc[i]) !== '') lines.push(inlineHtml(doc[i++].runs, false))
      if (lines.length) out.push(`<p>${lines.join('<br>')}</p>`)
      else i++
      continue
    }
    if (block.type === 'code') out.push(codeBlock(block.lang ?? '', blockText(block)))
    else if (block.type === 'table') out.push(tableHtml(blockText(block)))
    else if (block.type === 'rule') out.push('<hr>')
    else if (block.type === 'heading') out.push(`<h${block.level ?? 1}>${inlineHtml(block.runs, false)}</h${block.level ?? 1}>`)
    i++
  }
  return out.join('')
}

function tableEditorHtml(text: string): string {
  return text
    .split('\n')
    .map((line, i) => {
      const body = i === 1 && TABLE_DELIMITER.test(line) ? `<span data-pipe="">${escapeHtml(line)}</span>` : escapeHtml(line).replace(/\|/g, '<span data-pipe="">|</span>')
      return i === 0 && body ? `<span data-head="">${body}</span>` : body
    })
    .join('\n')
}

export function toEditorHtml(doc: Doc): string {
  const numbers = listNumbers(doc)
  return doc
    .map((block, i) => {
      const text = blockText(block)
      if (isRaw(block)) {
        const lang = block.lang ? ` data-lang="${escapeHtml(block.lang)}"` : ''
        const tail = text === '' || text.endsWith('\n') ? '<br data-ph="">' : ''
        const body = block.type === 'table' ? tableEditorHtml(text) : escapeHtml(text)
        return `<pre data-type="${block.type}"${lang}>${body}${tail}</pre>`
      }
      const attrs: string[] = [`data-type="${block.type}"`]
      if (block.type === 'heading') attrs.push(`data-level="${block.level ?? 1}"`)
      if (block.depth) attrs.push(`data-depth="${block.depth}"`, `style="--depth:${block.depth}"`)
      if (block.type === 'ordered') attrs.push(`data-n="${numbers[i]}"`)
      if (block.type === 'task') attrs.push(`data-checked="${block.checked ? 'true' : 'false'}"`)
      const html = inlineHtml(block.runs, true)
      return `<div ${attrs.join(' ')}>${html || '<br data-ph="">'}</div>`
    })
    .join('')
}

const BLOCK_TYPES = new Set<BlockType>(['paragraph', 'heading', 'quote', 'code', 'bullet', 'ordered', 'task', 'table', 'rule'])

function inlineFromDom(node: Node, code: boolean, marks: Mark[], href: string | undefined, out: Run[]) {
  if (node.nodeType === 3) {
    const text = (node.textContent ?? '').replace(/\u00a0/g, ' ').replace(/\u200b/g, '')
    if (text) out.push(code ? run(text) : run(text, marks, href))
    return
  }
  if (node.nodeType !== 1) return
  const el = node as Element
  if (el.tagName === 'BR') {
    if (code && !el.hasAttribute('data-ph')) out.push(run('\n'))
    return
  }
  const emoji = el.getAttribute('data-emoji')
  if (emoji !== null) {
    out.push(code ? run(emoji) : run(emoji, marks, href))
    return
  }
  const next = elementMarks(el, marks)
  if (el.tagName === 'A' && el.hasAttribute('data-image')) next.push('image')
  const link = el.tagName === 'A' ? el.getAttribute('href') ?? href : href
  for (const child of Array.from(el.childNodes)) inlineFromDom(child, code, next, link ?? undefined, out)
}

function elementMarks(el: Element, marks: Mark[]): Mark[] {
  const tag = el.tagName
  const style = (el as HTMLElement).style
  const set = new Set(marks)
  if (tag === 'STRONG' || (tag === 'B' && !/normal|^[1-5]00$/.test(style?.fontWeight ?? ''))) set.add('bold')
  if (tag === 'EM' || tag === 'I') set.add('italic')
  if (tag === 'U' || tag === 'INS') set.add('underline')
  if (tag === 'S' || tag === 'DEL' || tag === 'STRIKE') set.add('strike')
  if (tag === 'CODE' || tag === 'KBD' || tag === 'SAMP' || tag === 'TT') set.add('code')
  if (style) {
    if (/bold|[6-9]00/.test(style.fontWeight)) set.add('bold')
    if (style.fontStyle === 'italic') set.add('italic')
    if (/underline/.test(style.textDecoration || style.textDecorationLine || '')) set.add('underline')
    if (/line-through/.test(style.textDecoration || style.textDecorationLine || '')) set.add('strike')
  }
  return [...set]
}

export function fromEditorDom(root: Element): Doc {
  const doc: Doc = []
  for (const child of Array.from(root.childNodes)) {
    if (child.nodeType === 3) {
      const runs: Run[] = []
      inlineFromDom(child, false, [], undefined, runs)
      doc.push(blockOf('paragraph', normalizeRuns(runs)))
      continue
    }
    if (child.nodeType !== 1) continue
    const el = child as Element
    if (el.tagName === 'BR') {
      doc.push(blockOf('paragraph', []))
      continue
    }
    const attr = el.getAttribute('data-type') as BlockType | null
    const type: BlockType = attr && BLOCK_TYPES.has(attr) ? attr : el.tagName === 'PRE' ? 'code' : 'paragraph'
    const raw = RAW.has(type)
    const runs: Run[] = []
    for (const node of Array.from(el.childNodes)) inlineFromDom(node, raw, [], undefined, runs)
    doc.push(
      blockOf(type, normalizeRuns(runs), {
        lang: el.getAttribute('data-lang') ?? undefined,
        level: Number(el.getAttribute('data-level')) || 1,
        depth: Number(el.getAttribute('data-depth')) || 0,
        checked: el.getAttribute('data-checked') === 'true'
      })
    )
  }
  return doc.length ? doc : emptyDoc()
}

const BLOCK_TAGS = new Set(['P', 'DIV', 'SECTION', 'ARTICLE', 'HEADER', 'FOOTER', 'TR', 'DL', 'DT', 'DD', 'FIGURE'])
const SKIP_TAGS = new Set(['SCRIPT', 'STYLE', 'HEAD', 'META', 'TITLE', 'NOSCRIPT', 'TEMPLATE'])

function tableText(table: Element): string {
  const rows = Array.from(table.querySelectorAll('tr')).map((tr) => Array.from(tr.children).map((cell) => (cell.textContent ?? '').replace(/\s+/g, ' ').replace(/\|/g, '\\|').trim()))
  if (!rows.length) return ''
  const width = Math.max(...rows.map((r) => r.length))
  const line = (cells: string[]) => `| ${Array.from({ length: width }, (_, k) => cells[k] ?? '').join(' | ')} |`
  return [line(rows[0]), line(Array.from({ length: width }, () => '---')), ...rows.slice(1).map(line)].join('\n')
}

export function htmlToDoc(html: string): Doc {
  const parsed = new DOMParser().parseFromString(html, 'text/html')
  const doc: Doc = []
  let current: Block | null = null

  const finish = () => {
    if (current) {
      current.runs = trimRuns(normalizeRuns(current.runs))
      doc.push(current)
    }
    current = null
  }

  const walk = (node: Node, type: BlockType, attrs: Partial<Block>, list: BlockType | null, marks: Mark[], href: string | undefined) => {
    if (node.nodeType === 3) {
      const text = (node.textContent ?? '').replace(/\s+/g, ' ')
      if (!current) {
        if (!text.trim()) return
        current = blockOf(type, [], attrs)
      }
      current.runs.push(run(text, marks, href))
      return
    }
    if (node.nodeType !== 1) return
    const el = node as Element
    const tag = el.tagName
    if (SKIP_TAGS.has(tag)) return
    if (tag === 'BR') {
      if (!current) current = blockOf(type, [], attrs)
      finish()
      return
    }
    if (tag === 'HR') {
      finish()
      doc.push(blockOf('rule', []))
      return
    }
    if (tag === 'IMG') {
      const src = el.getAttribute('src') ?? ''
      if (!/^https?:\/\//i.test(src)) return
      if (!current) current = blockOf(type, [], attrs)
      current.runs.push(run(el.getAttribute('alt') || imageName(src), [...marks, 'image'], src))
      return
    }
    if (tag === 'TABLE') {
      finish()
      const text = tableText(el)
      if (text) doc.push(blockOf('table', [run(text)]))
      return
    }
    if (tag === 'PRE') {
      finish()
      const text = (el.textContent ?? '').replace(/\u00a0/g, ' ').replace(/\n$/, '')
      doc.push(blockOf('code', normalizeRuns([run(text)])))
      return
    }
    if (/^H[1-6]$/.test(tag)) {
      finish()
      for (const child of Array.from(el.childNodes)) walk(child, 'heading', { level: Number(tag[1]) }, list, marks, href)
      finish()
      return
    }
    if (tag === 'UL' || tag === 'OL') {
      finish()
      const depth = list ? (attrs.depth ?? 0) + 1 : 0
      for (const child of Array.from(el.childNodes)) walk(child, type, { depth }, tag === 'UL' ? 'bullet' : 'ordered', marks, href)
      finish()
      return
    }
    if (tag === 'LI') {
      finish()
      const box = el.querySelector(':scope > input[type="checkbox"]') as HTMLInputElement | null
      const kind: BlockType = box ? 'task' : (list ?? 'bullet')
      for (const child of Array.from(el.childNodes)) walk(child, kind, { depth: attrs.depth, checked: box?.checked }, list, marks, href)
      finish()
      return
    }
    if (tag === 'INPUT') return
    if (tag === 'BLOCKQUOTE') {
      finish()
      for (const child of Array.from(el.childNodes)) walk(child, 'quote', {}, list, marks, href)
      finish()
      return
    }
    const block = BLOCK_TAGS.has(tag)
    if (block) finish()
    const next = elementMarks(el, marks)
    const link = tag === 'A' ? el.getAttribute('href') ?? href : href
    for (const child of Array.from(el.childNodes)) walk(child, type, attrs, list, next, link ?? undefined)
    if (block) finish()
  }

  for (const child of Array.from(parsed.body.childNodes)) walk(child, 'paragraph', {}, null, [], undefined)
  finish()
  const blank = (b: Block) => !isRaw(b) && b.type !== 'rule' && blockText(b) === ''
  while (doc.length && blank(doc[0])) doc.shift()
  while (doc.length && blank(doc[doc.length - 1])) doc.pop()
  return doc.length ? doc.map((b) => (isRaw(b) ? b : { ...b, runs: linkify(b.runs) })) : emptyDoc()
}

function trimRuns(runs: Run[]): Run[] {
  if (!runs.length) return runs
  const out = runs.map((r) => run(r.text, r.marks, r.href))
  out[0].text = out[0].text.replace(/^\s+/, '')
  out[out.length - 1].text = out[out.length - 1].text.replace(/\s+$/, '')
  return normalizeRuns(out)
}
