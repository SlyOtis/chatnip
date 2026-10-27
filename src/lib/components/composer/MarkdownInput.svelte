<script lang="ts">
  import { anchored, boxOf } from '$lib/popover'
  import { flushSync, onMount, tick } from 'svelte'
  import { Field, IconButton } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'
  import { shortcodeQuery } from '$lib/emoji'
  import {
    applyRules,
    backspaceAtStart,
    blockText,
    cloneDoc,
    deleteRange,
    docText,
    emptyDoc,
    enter,
    enterAction,
    exitCode,
    fromEditorDom,
    fromMarkdown,
    hasMarkdown,
    htmlToDoc,
    indent,
    insertBlock,
    insertDoc,
    insertText as insertAt,
    isEmpty,
    isRaw,
    linkAt,
    locate,
    looksLikeMarkdown,
    markdownMap,
    marksAt,
    mentionRun,
    channelRun,
    normalizeRuns,
    setBlocks,
    setMark,
    sourceOffset,
    TABLE_TEMPLATE,
    textToDoc,
    toEditorHtml,
    toggleMark,
    toggleTask,
    visibleOffset,
    blockOf,
    run,
    type Block,
    type BlockType,
    type Doc,
    type Mark
  } from './markdown'

  interface Props {
    doc?: Doc
    placeholder?: string
    maxLines?: number
    fill?: boolean
    submitOnEnter?: boolean
    onsubmit?: (andDone: boolean) => void
    onmention?: (query: string | null) => void
    onchannel?: (query: string | null) => void
    onshortcode?: (query: string | null) => void
    onfiles?: (files: File[]) => void
    onkeydown?: (e: KeyboardEvent) => void
  }

  let { doc = $bindable(emptyDoc()), placeholder = '', maxLines = 10, fill = false, submitOnEnter = true, onsubmit, onmention, onchannel, onshortcode, onfiles, onkeydown }: Props = $props()

  interface Snapshot {
    doc: Doc
    from: number
    to: number
  }

  let root = $state<HTMLDivElement>()
  let linkBox = $state<HTMLDivElement>()
  let linkOpen = $state(false)
  let linkValue = $state('')
  let linkRange = { from: 0, to: 0 }
  let pendingMarks: Mark[] | null = null
  let pendingAt = -1
  let composing = false
  let lastSel = { from: 0, to: 0 }
  let history: Snapshot[] = []
  let historyIndex = -1
  let lastPush = 0
  let raw = $state(false)
  let source = $state('')
  let sourceOpened = ''
  let sourceBox = $state<HTMLTextAreaElement>()

  const detected = $derived(hasMarkdown(doc))
  const empty = $derived(doc.length === 1 && doc[0].type === 'paragraph' && blockText(doc[0]) === '')

  function count(node: Node, code: boolean): number {
    if (node.nodeType === 3) return (node.textContent ?? '').length
    if (node.nodeType === 1 && (node as Element).hasAttribute('data-emoji')) return ((node as Element).getAttribute('data-emoji') ?? '').length
    if (node.nodeType === 1 && (node as Element).tagName === 'BR') return code && !(node as Element).hasAttribute('data-ph') ? 1 : 0
    let n = 0
    for (const child of Array.from(node.childNodes)) n += count(child, code)
    return n
  }

  function isCode(block: Node): boolean {
    return block.nodeType === 1 && ((block as Element).getAttribute('data-type') === 'code' || (block as Element).tagName === 'PRE')
  }

  function offsetOf(node: Node, offset: number): number {
    if (!root) return 0
    const blocks = Array.from(root.childNodes)
    if (node === root) {
      let pos = 0
      for (let i = 0; i < offset && i < blocks.length; i++) pos += count(blocks[i], isCode(blocks[i])) + 1
      return Math.max(0, offset >= blocks.length ? pos - 1 : pos)
    }
    let block: Node | null = node
    while (block && block.parentNode !== root) block = block.parentNode
    if (!block) return 0
    let pos = 0
    for (const b of blocks) {
      if (b === block) break
      pos += count(b, isCode(b)) + 1
    }
    const range = document.createRange()
    range.setStart(block, 0)
    range.setEnd(node, offset)
    return pos + count(range.cloneContents(), isCode(block))
  }

  function readSel(): { from: number; to: number } {
    const sel = window.getSelection()
    if (!root || !sel || !sel.rangeCount || !root.contains(sel.anchorNode)) return lastSel
    const a = offsetOf(sel.anchorNode!, sel.anchorOffset)
    const b = offsetOf(sel.focusNode!, sel.focusOffset)
    return { from: Math.min(a, b), to: Math.max(a, b) }
  }

  function pointAt(pos: number): [Node, number] {
    const { index, offset } = locate(doc, pos)
    const block = root!.childNodes[index]
    if (!block) return [root!, root!.childNodes.length]
    const code = isCode(block)
    let rest = offset
    const walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT)
    let node: Node | null = block.nodeType === 3 ? block : walker.nextNode()
    while (node) {
      const emoji = node.nodeType === 1 && (node as Element).hasAttribute('data-emoji') ? ((node as Element).getAttribute('data-emoji') ?? '') : null
      if (emoji !== null) {
        const at = Array.from(node.parentNode!.childNodes).indexOf(node as ChildNode)
        if (rest === 0) return [node.parentNode!, at]
        if (rest <= emoji.length) return [node.parentNode!, at + 1]
        rest -= emoji.length
        walker.currentNode = node.lastChild ?? node
        while (walker.currentNode.lastChild) walker.currentNode = walker.currentNode.lastChild
      } else if (node.nodeType === 3) {
        const len = (node.textContent ?? '').length
        if (rest <= len) return [node, rest]
        rest -= len
      } else if ((node as Element).tagName === 'BR' && code && !(node as Element).hasAttribute('data-ph')) {
        if (rest === 0) return [node.parentNode!, Array.from(node.parentNode!.childNodes).indexOf(node as ChildNode)]
        rest -= 1
        if (rest === 0) return [node.parentNode!, Array.from(node.parentNode!.childNodes).indexOf(node as ChildNode) + 1]
      }
      node = walker.nextNode()
    }
    return [block, block.childNodes.length && (block.lastChild as Element).tagName === 'BR' ? block.childNodes.length - 1 : block.childNodes.length]
  }

  function writeSel(from: number, to = from) {
    if (!root || document.activeElement !== root) {
      lastSel = { from, to }
      return
    }
    const sel = window.getSelection()
    if (!sel) return
    const [a, ao] = pointAt(from)
    const [b, bo] = pointAt(to)
    sel.setBaseAndExtent(a, ao, b, bo)
    lastSel = { from, to }
  }

  function render() {
    if (!root) return
    const html = toEditorHtml(doc)
    if (root.innerHTML !== html) root.innerHTML = html
  }

  function remember(from: number, to: number, coalesce: boolean) {
    const now = Date.now()
    const snap = { doc: cloneDoc(doc), from, to }
    if (coalesce && historyIndex > 0 && now - lastPush < 800) history[historyIndex] = snap
    else {
      history = history.slice(0, historyIndex + 1)
      history.push(snap)
      historyIndex = history.length - 1
      if (history.length > 200) {
        history.shift()
        historyIndex--
      }
    }
    lastPush = now
  }

  function mention(pos: number) {
    const { index, offset } = locate(doc, pos)
    const block = doc[index]
    const before = isRaw(block) ? null : blockText(block).slice(0, offset)
    onshortcode?.(before === null ? null : (shortcodeQuery(before)?.query ?? null))
    if (onchannel) {
      const c = before === null || block.type === 'code' ? null : /(?:^|\s)#([\w-]*)$/.exec(before)
      onchannel(c ? c[1] : null)
    }
    if (!onmention) return
    const m = block.type === 'code' ? null : /(?:^|\s)@(\w*)$/.exec(blockText(block).slice(0, offset))
    onmention(m ? m[1] : null)
  }

  function commit(next: Doc, from: number, to = from, coalesce = false) {
    doc = next.length ? next : emptyDoc()
    render()
    writeSel(from, to)
    remember(from, to, coalesce)
    mention(to)
  }

  function type(text: string) {
    const sel = readSel()
    let base = doc
    if (sel.from !== sel.to) base = deleteRange(base, sel.from, sel.to).doc
    const marks = pendingMarks && pendingAt === sel.from ? pendingMarks : marksAt(base, sel.from)
    pendingMarks = null
    const inserted = insertAt(base, sel.from, text, marks)
    const ruled = applyRules(inserted.doc, inserted.pos)
    if (ruled.fired) {
      pendingMarks = []
      pendingAt = ruled.pos
    }
    commit(ruled.doc, ruled.pos, ruled.pos, !ruled.fired && text.length === 1 && sel.from === sel.to)
  }

  function paste(fragment: Doc) {
    const sel = readSel()
    const base = sel.from !== sel.to ? deleteRange(doc, sel.from, sel.to).doc : doc
    const r = insertDoc(base, sel.from, fragment)
    commit(r.doc, r.pos)
  }

  function mark(m: Mark) {
    const sel = readSel()
    if (sel.from === sel.to) {
      const current = pendingMarks && pendingAt === sel.from ? pendingMarks : marksAt(doc, sel.from)
      pendingMarks = current.includes(m) ? current.filter((x) => x !== m) : [...current, m]
      pendingAt = sel.from
      return
    }
    commit(toggleMark(doc, sel.from, sel.to, m), sel.from, sel.to)
  }

  async function openLink() {
    const sel = readSel()
    linkRange = sel
    linkValue = linkAt(doc, sel.from) ?? ''
    linkOpen = true
    await tick()
    const field = linkBox?.querySelector('input')
    field?.focus()
    field?.select()
  }

  function closeLink() {
    linkOpen = false
    root?.focus()
    writeSel(linkRange.from, linkRange.to)
  }

  function applyLink() {
    const href = linkValue.trim()
    const { from, to } = linkRange
    linkOpen = false
    root?.focus()
    if (!href && from === to) return writeSel(from)
    if (from === to) {
      const r = insertAt(doc, from, href, [], href)
      commit(r.doc, r.pos)
      pendingMarks = []
      pendingAt = r.pos
      return
    }
    commit(setMark(doc, from, to, 'link', !!href, href || undefined), to)
  }

  function undo(redo: boolean) {
    const target = historyIndex + (redo ? 1 : -1)
    const snap = history[target]
    if (!snap) return
    historyIndex = target
    doc = cloneDoc(snap.doc)
    render()
    writeSel(snap.from, snap.to)
  }

  function handleKeydown(e: KeyboardEvent) {
    onkeydown?.(e)
    if (e.defaultPrevented || composing || e.isComposing || e.keyCode === 229) return
    const mod = e.metaKey || e.ctrlKey
    const key = e.key.toLowerCase()
    if (e.key === 'Enter') {
      const sel = readSel()
      const action = enterAction(doc[locate(doc, sel.from).index], e, submitOnEnter, composing)
      if (action === 'compose') return
      e.preventDefault()
      if (action !== 'edit') {
        onsubmit?.(action === 'send-done')
        return
      }
      const base = sel.from !== sel.to ? deleteRange(doc, sel.from, sel.to).doc : doc
      const r = enter(base, sel.from)
      commit(r.doc, r.pos)
      return
    }
    if (e.key === 'Backspace' && !mod && !e.altKey) {
      const sel = readSel()
      if (sel.from !== sel.to) return
      const r = backspaceAtStart(doc, sel.from)
      if (r) {
        e.preventDefault()
        commit(r.doc, r.pos)
      }
      return
    }
    if (e.key === 'ArrowDown' && !mod && !e.shiftKey) {
      const sel = readSel()
      const r = sel.from === sel.to ? exitCode(doc, sel.from) : null
      if (r) {
        e.preventDefault()
        commit(r.doc, r.pos)
      }
      return
    }
    if (e.key === 'Tab' && !mod && !e.altKey) {
      const sel = readSel()
      const next = indent(doc, sel.from, sel.to, e.shiftKey ? -1 : 1)
      if (next) {
        e.preventDefault()
        commit(next, sel.from, sel.to)
      } else if (isRaw(doc[locate(doc, sel.from).index]) && !e.shiftKey) {
        e.preventDefault()
        type('  ')
      }
      return
    }
    if (e.key === 'Escape') {
      root?.blur()
      return
    }
    if (mod && e.shiftKey && !e.altKey && (e.code === 'Digit7' || e.code === 'Digit8' || e.code === 'Digit9')) {
      e.preventDefault()
      block(e.code === 'Digit7' ? 'ordered' : e.code === 'Digit8' ? 'bullet' : 'quote')
      return
    }
    if (mod && e.shiftKey && e.altKey && e.code === 'KeyC') {
      e.preventDefault()
      block('code')
      return
    }
    if (!mod || e.altKey) return
    if (key === 'b' && !e.shiftKey) {
      e.preventDefault()
      mark('bold')
    } else if (key === 'i' && !e.shiftKey) {
      e.preventDefault()
      mark('italic')
    } else if (key === 'u' && !e.shiftKey) {
      e.preventDefault()
      mark('underline')
    } else if (key === 'x' && e.shiftKey) {
      e.preventDefault()
      mark('strike')
    } else if (key === 'e' && !e.shiftKey) {
      e.preventDefault()
      mark('code')
    } else if (key === 'k' && !e.shiftKey) {
      e.preventDefault()
      e.stopPropagation()
      openLink()
    } else if (key === 'm' && e.shiftKey) {
      e.preventDefault()
      openSource()
    } else if (key === 'v' && e.shiftKey) {
      e.preventDefault()
      navigator.clipboard?.readText().then((text) => text && paste(textToDoc(text))).catch(() => {})
    } else if (key === 'z') {
      e.preventDefault()
      undo(e.shiftKey)
    } else if (key === 'y') {
      e.preventDefault()
      undo(true)
    }
  }

  function handleBeforeInput(e: InputEvent) {
    if (composing || e.isComposing) return
    switch (e.inputType) {
      case 'insertText':
      case 'insertReplacementText': {
        const text = e.data ?? e.dataTransfer?.getData('text/plain') ?? ''
        if (!text) return
        e.preventDefault()
        type(text)
        return
      }
      case 'insertParagraph':
      case 'insertLineBreak': {
        e.preventDefault()
        const sel = readSel()
        const r = enter(sel.from !== sel.to ? deleteRange(doc, sel.from, sel.to).doc : doc, sel.from)
        commit(r.doc, r.pos)
        return
      }
      case 'formatBold':
        e.preventDefault()
        return mark('bold')
      case 'formatItalic':
        e.preventDefault()
        return mark('italic')
      case 'formatUnderline':
        e.preventDefault()
        return mark('underline')
      case 'formatStrikeThrough':
        e.preventDefault()
        return mark('strike')
      case 'historyUndo':
        e.preventDefault()
        return undo(false)
      case 'historyRedo':
        e.preventDefault()
        return undo(true)
    }
  }

  function sync() {
    if (!root) return
    const sel = readSel()
    commit(fromEditorDom(root), sel.from, sel.to, true)
  }

  function handleInput() {
    if (composing) return
    sync()
  }

  function handlePaste(e: ClipboardEvent) {
    const data = e.clipboardData
    if (!data) return
    const files = Array.from(data.files ?? [])
    if (files.length) {
      e.preventDefault()
      onfiles?.(files)
      return
    }
    e.preventDefault()
    const sel = readSel()
    const inCode = isRaw(doc[locate(doc, sel.from).index])
    const html = data.getData('text/html')
    const text = data.getData('text/plain')
    if (inCode) {
      const base = sel.from !== sel.to ? deleteRange(doc, sel.from, sel.to).doc : doc
      const r = insertAt(base, sel.from, text.replace(/\r\n?/g, '\n'))
      commit(r.doc, r.pos)
      return
    }
    paste(looksLikeMarkdown(text) ? fromMarkdown(text) : html ? htmlToDoc(html) : textToDoc(text))
  }

  function handleMousedown(e: MouseEvent) {
    if (!root || e.button !== 0) return
    const target = (e.target as Element | null)?.closest?.('[data-type="task"]')
    if (!target || target.parentNode !== root) return
    const box = target.getBoundingClientRect()
    const inset = parseFloat(getComputedStyle(target).paddingLeft) || 0
    if (e.clientX > box.left + inset) return
    e.preventDefault()
    const index = Array.from(root.childNodes).indexOf(target as ChildNode)
    const sel = readSel()
    commit(toggleTask(doc, index), sel.from, sel.to)
  }

  function handleSelectionChange() {
    const sel = window.getSelection()
    if (root && sel?.anchorNode && root.contains(sel.anchorNode)) lastSel = readSel()
  }

  function openSource() {
    const sel = readSel()
    const map = markdownMap(doc)
    source = map.text
    sourceOpened = source
    flushSync(() => (raw = true))
    sourceBox?.focus()
    sourceBox?.setSelectionRange(sourceOffset(map, sel.from), sourceOffset(map, sel.to))
    onmention?.(null)
    onchannel?.(null)
    onshortcode?.(null)
  }

  function closeSource() {
    const start = sourceBox?.selectionStart ?? source.length
    const end = sourceBox?.selectionEnd ?? start
    const changed = source !== sourceOpened
    const next = changed ? fromMarkdown(source) : doc
    const map = markdownMap(next)
    const from = visibleOffset(map, start)
    const to = visibleOffset(map, end)
    flushSync(() => (raw = false))
    root?.focus()
    if (changed) commit(next, from, to)
    else {
      render()
      writeSel(from, to)
    }
  }

  function putSource(value: string, caret: number) {
    flushSync(() => (source = value))
    doc = fromMarkdown(value)
    sourceBox?.focus()
    sourceBox?.setSelectionRange(caret, caret)
  }

  function insertSource(text: string, back = 0) {
    const end = sourceBox?.selectionEnd ?? source.length
    const start = Math.max(0, (sourceBox?.selectionStart ?? source.length) - back)
    putSource(source.slice(0, start) + text + source.slice(end), start + text.length)
  }

  function handleSourceInput() {
    if (!sourceBox) return
    source = sourceBox.value
    doc = fromMarkdown(source)
    const m = /(?:^|\s)@(\w*)$/.exec(source.slice(0, sourceBox.selectionStart))
    onmention?.(m ? m[1] : null)
    onshortcode?.(shortcodeQuery(source.slice(0, sourceBox.selectionStart))?.query ?? null)
    const c = /(?:^|\s)#([\w-]*)$/.exec(source.slice(0, sourceBox.selectionStart))
    onchannel?.(c ? c[1] : null)
  }

  function handleSourceKeydown(e: KeyboardEvent) {
    onkeydown?.(e)
    if (e.defaultPrevented) return
    const mod = e.metaKey || e.ctrlKey
    if (mod && e.shiftKey && e.key.toLowerCase() === 'm') {
      e.preventDefault()
      closeSource()
    } else if (e.key === 'Enter' && mod) {
      e.preventDefault()
      onsubmit?.(e.shiftKey)
    } else if (e.key === 'Escape') {
      sourceBox?.blur()
    }
  }

  export function toggleSource() {
    if (raw) closeSource()
    else openSource()
  }

  export function isSource(): boolean {
    return raw
  }

  export function focus() {
    if (raw) return sourceBox?.focus()
    root?.focus()
    writeSel(lastSel.from, lastSel.to)
  }

  export function format(kind: 'bold' | 'italic' | 'underline' | 'strike' | 'code' | 'link') {
    if (raw) closeSource()
    root?.focus()
    writeSel(lastSel.from, lastSel.to)
    if (kind === 'link') openLink()
    else mark(kind)
  }

  export function block(kind: BlockType, attrs: Partial<Block> = {}) {
    if (raw) closeSource()
    root?.focus()
    writeSel(lastSel.from, lastSel.to)
    const sel = readSel()
    if (kind === 'table' || kind === 'rule') {
      const r = kind === 'table' ? insertBlock(doc, sel.to, blockOf('table', [run(TABLE_TEMPLATE)]), 2) : insertBlock(doc, sel.to, blockOf('rule', []))
      commit(r.doc, r.pos)
      return
    }
    commit(setBlocks(doc, sel.from, sel.to, kind, attrs), sel.from, sel.to)
  }

  export function insertMention(length: number, id: string, name: string) {
    if (raw) return insertSource(`[@${name}](mention:${id}) `, length)
    root?.focus()
    const sel = readSel()
    const from = Math.max(0, sel.from - length)
    const base = deleteRange(doc, from, sel.to).doc
    const r = insertDoc(base, from, [blockOf('paragraph', normalizeRuns([mentionRun(id, name), run(' ')]))])
    pendingMarks = []
    pendingAt = r.pos
    commit(r.doc, r.pos)
  }

  export function insertChannel(length: number, id: string, name: string) {
    if (raw) return insertSource(`[#${name}](channel:${id}) `, length)
    root?.focus()
    const sel = readSel()
    const from = Math.max(0, sel.from - length)
    const base = deleteRange(doc, from, sel.to).doc
    const r = insertDoc(base, from, [blockOf('paragraph', normalizeRuns([channelRun(id, name), run(' ')]))])
    pendingMarks = []
    pendingAt = r.pos
    commit(r.doc, r.pos)
  }

  export function insertShortcode(length: number, code: string) {
    if (raw) return insertSource(`${code} `, length)
    root?.focus()
    const sel = readSel()
    const from = Math.max(0, sel.from - length)
    const base = deleteRange(doc, from, sel.to).doc
    const r = insertAt(base, from, `${code} `, marksAt(base, from))
    commit(r.doc, r.pos)
  }

  export function insertMarkdown(text: string) {
    if (raw) return insertSource(text)
    root?.focus()
    writeSel(lastSel.from, lastSel.to)
    paste(fromMarkdown(text))
  }

  export function replaceText(previous: string, next: string): boolean {
    if (raw) {
      const at = previous ? source.lastIndexOf(previous) : -1
      if (at >= 0) putSource(source.slice(0, at) + next + source.slice(at + previous.length), at + next.length)
      return at >= 0
    }
    const at = previous ? docText(doc).lastIndexOf(previous) : -1
    if (at < 0) return false
    const r = insertAt(deleteRange(doc, at, at + previous.length).doc, at, next)
    commit(r.doc, r.pos)
    return true
  }

  export function insertText(text: string) {
    if (raw) return insertSource(text)
    root?.focus()
    writeSel(lastSel.from, lastSel.to)
    const sel = readSel()
    const base = sel.from !== sel.to ? deleteRange(doc, sel.from, sel.to).doc : doc
    const r = insertAt(base, sel.from, text, marksAt(base, sel.from))
    commit(r.doc, r.pos)
  }

  export function clear() {
    if (raw) flushSync(() => (raw = false))
    source = ''
    doc = emptyDoc()
    render()
    history = []
    historyIndex = -1
    remember(0, 0, false)
    writeSel(0)
    onmention?.(null)
    onchannel?.(null)
    onshortcode?.(null)
  }

  export function isBlank(): boolean {
    return isEmpty(doc)
  }

  onMount(() => {
    render()
    remember(0, 0, false)
    document.addEventListener('selectionchange', handleSelectionChange)
    return () => document.removeEventListener('selectionchange', handleSelectionChange)
  })
</script>

<div class="relative {fill ? 'flex h-full min-h-0 flex-col' : ''}">
  {#if linkOpen}
    <div bind:this={linkBox} data-testid="composer-link" class="menu flex w-80 items-center gap-2" use:anchored={{ at: () => boxOf(root ?? null), side: 'above', align: 'start', onclose: () => (linkOpen = false) }}>
      <Field
        bind:value={linkValue}
        size="sm"
        mono
        placeholder="https://"
        aria-label={t('composer.link')}
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            applyLink()
          } else if (e.key === 'Escape') {
            e.preventDefault()
            e.stopPropagation()
            closeLink()
          }
        }}
        onblur={() => (linkOpen = false)}
      />
    </div>
  {/if}
  {#if detected || raw}
    <div class="absolute right-0 top-0 z-[1]">
      <IconButton icon="lucide:code-xml" testid="composer-source-toggle" label={t('composer.markdownSource')} keys="⌘⇧M" size="sm" active={raw} onclick={toggleSource} />
    </div>
  {/if}
  {#if empty && !raw}
    <span class="pointer-events-none absolute left-0 top-0 select-none text-[14px] leading-6 text-ink-3">{placeholder}</span>
  {/if}
  <div
    bind:this={root}
    data-composer
    data-testid="composer-input"
    contenteditable="true"
    role="textbox"
    tabindex="0"
    aria-multiline="true"
    aria-label={placeholder || t('composer.message')}
    spellcheck="true"
    class="md min-h-6 w-full overflow-y-auto whitespace-pre-wrap break-words text-[14px] leading-6 text-ink outline-none focus-visible:outline-none {fill ? 'min-h-0 flex-1' : ''} {raw ? 'hidden' : ''} {detected ? 'pr-8' : ''}"
    style:max-height={fill ? undefined : `${maxLines * 1.5}rem`}
    onkeydown={handleKeydown}
    onmousedown={handleMousedown}
    onbeforeinput={handleBeforeInput}
    oninput={handleInput}
    onpaste={handlePaste}
    oncompositionstart={() => (composing = true)}
    oncompositionend={() => {
      composing = false
      sync()
    }}
  ></div>
  {#if raw}
    <textarea
      bind:this={sourceBox}
      data-testid="composer-source"
      value={source}
      {placeholder}
      aria-label={t('composer.markdownSource')}
      spellcheck="false"
      rows={Math.min(maxLines, source.split('\n').length)}
      class="min-h-6 w-full resize-none bg-transparent pr-8 font-mono text-[13px] leading-6 text-ink outline-none placeholder:text-ink-3 {fill ? 'min-h-0 flex-1' : ''}"
      oninput={handleSourceInput}
      onkeydown={handleSourceKeydown}
    ></textarea>
  {/if}
</div>

<style>
  .md :global([data-type='quote']) {
    border-left: 2px solid var(--color-line-strong, currentColor);
    padding-left: 0.75rem;
    color: var(--color-ink-2, inherit);
  }
  .md :global([data-type='heading']) {
    font-weight: 650;
    letter-spacing: -0.01em;
    color: var(--color-ink, inherit);
  }
  .md :global([data-type='heading'][data-level='1']) {
    font-size: 1.375em;
    line-height: 1.4;
  }
  .md :global([data-type='heading'][data-level='2']) {
    font-size: 1.2em;
    line-height: 1.45;
  }
  .md :global([data-type='heading'][data-level='3']) {
    font-size: 1.08em;
  }
  .md :global([data-type='bullet']),
  .md :global([data-type='ordered']),
  .md :global([data-type='task']) {
    position: relative;
    padding-left: calc(1.5rem * (var(--depth, 0) + 1));
  }
  .md :global([data-type='bullet'])::before,
  .md :global([data-type='ordered'])::before,
  .md :global([data-type='task'])::before {
    position: absolute;
    left: calc(1.5rem * var(--depth, 0));
    width: 1.25rem;
    color: var(--color-ink-3, inherit);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .md :global([data-type='bullet'])::before {
    content: '•';
    text-align: center;
  }
  .md :global([data-type='bullet'][data-depth='1'])::before,
  .md :global([data-type='bullet'][data-depth='4'])::before {
    content: '◦';
  }
  .md :global([data-type='bullet'][data-depth='2'])::before,
  .md :global([data-type='bullet'][data-depth='5'])::before {
    content: '▪';
  }
  .md :global([data-type='ordered'])::before {
    content: attr(data-n) '.';
  }
  .md :global([data-type='task'])::before {
    content: '';
    top: 0.3125rem;
    left: calc(1.5rem * var(--depth, 0) + 0.3125rem);
    width: 0.875rem;
    height: 0.875rem;
    border: 1px solid var(--color-line-strong, currentColor);
    border-radius: 0.25rem;
    cursor: pointer;
  }
  .md :global([data-type='task'][data-checked='true']) {
    color: var(--color-ink-3, inherit);
    text-decoration: line-through;
  }
  .md :global([data-type='task'][data-checked='true'])::before {
    border-color: var(--color-accent, currentColor);
    background: var(--color-accent, currentColor) url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='white' stroke-width='3' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M20 6 9 17l-5-5'/%3E%3C/svg%3E") center / 80% no-repeat;
  }
  .md :global([data-type='rule']) {
    background: linear-gradient(var(--color-line-strong, currentColor), var(--color-line-strong, currentColor)) center / 100% 1px no-repeat;
    caret-color: transparent;
  }
  .md :global(pre[data-type='code']),
  .md :global(pre[data-type='table']) {
    position: relative;
    margin: 0.25rem 0;
    padding: 0.25rem 0.5rem;
    border-radius: 0.375rem;
    background: var(--color-paper-3, transparent);
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 13px;
    white-space: pre-wrap;
  }
  .md :global(pre[data-lang])::after {
    content: attr(data-lang);
    position: absolute;
    top: 0.25rem;
    right: 0.5rem;
    font-size: 11px;
    color: var(--color-ink-3, inherit);
    pointer-events: none;
  }
  .md :global(pre[data-type='table']) {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--color-line, transparent);
  }
  .md :global(pre [data-head]) {
    font-weight: 600;
  }
  .md :global(pre [data-pipe]) {
    color: var(--color-ink-3, inherit);
    font-weight: 400;
  }
  .md :global(code) {
    padding: 0 0.25rem;
    border-radius: 0.25rem;
    background: var(--color-paper-3, transparent);
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 0.9em;
  }
  .md :global(a) {
    color: var(--color-accent, inherit);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .md :global(a[data-mention]) {
    padding: 0 0.125rem;
    border-radius: 0.25rem;
    background: var(--color-accent-soft, transparent);
    font-weight: 500;
    text-decoration: none;
  }
  .md :global(a[data-image]) {
    padding: 0 0.25rem;
    border-radius: 0.25rem;
    background: var(--color-paper-3, transparent);
    color: var(--color-ink-2, inherit);
    text-decoration: none;
  }
  .md :global(a[data-image])::before {
    content: '';
    display: inline-block;
    width: 0.9em;
    height: 0.9em;
    margin-right: 0.25em;
    vertical-align: -0.1em;
    background: currentColor;
    mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Crect width='18' height='18' x='3' y='3' rx='2'/%3E%3Ccircle cx='9' cy='9' r='2'/%3E%3Cpath d='m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21'/%3E%3C/svg%3E") center / contain no-repeat;
  }
  .md :global(strong) {
    font-weight: 600;
  }
</style>
