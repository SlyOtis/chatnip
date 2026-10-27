import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render } from '@testing-library/svelte'
import { tick } from 'svelte'
import MarkdownInput from './MarkdownInput.svelte'

afterEach(() => cleanup())

function mount() {
  const result = render(MarkdownInput, { props: { submitOnEnter: false } })
  const root = result.container.querySelector('[data-composer]') as HTMLDivElement
  root.focus()
  return { root, editor: result.component as unknown as { format: (k: string) => void; block: (k: string, a?: Record<string, unknown>) => void; insertMarkdown: (t: string) => void; insertMention: (l: number, id: string, name: string) => void } }
}

function type(root: HTMLElement, text: string) {
  for (const data of text) root.dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertText', data, bubbles: true, cancelable: true }))
}

function key(root: HTMLElement, init: KeyboardEventInit) {
  root.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }))
}

function selectAll(root: HTMLElement) {
  window.getSelection()?.selectAllChildren(root)
  document.dispatchEvent(new Event('selectionchange'))
}

describe('MarkdownInput live rendering', () => {
  it('renders a typed heading, quote, list and inline marks as they are typed', async () => {
    const { root } = mount()
    type(root, '# Plan')
    await tick()
    expect(root.querySelector('[data-type="heading"][data-level="1"]')?.textContent).toBe('Plan')
    key(root, { key: 'Enter', shiftKey: true })
    type(root, '> sitat **fet**')
    await tick()
    expect(root.querySelector('[data-type="quote"] strong')?.textContent).toBe('fet')
    key(root, { key: 'Enter', shiftKey: true })
    key(root, { key: 'Enter', shiftKey: true })
    type(root, '- [ ] gjør')
    await tick()
    expect(root.querySelector('[data-type="task"][data-checked="false"]')?.textContent).toBe('gjør')
  })

  it('nests a list item with Tab and outdents with Shift+Tab', async () => {
    const { root } = mount()
    type(root, '- a')
    key(root, { key: 'Enter', shiftKey: true })
    type(root, 'b')
    key(root, { key: 'Tab' })
    await tick()
    expect(root.querySelectorAll('[data-type="bullet"]')[1].getAttribute('data-depth')).toBe('1')
    key(root, { key: 'Tab', shiftKey: true })
    await tick()
    expect(root.querySelectorAll('[data-type="bullet"]')[1].hasAttribute('data-depth')).toBe(false)
  })

  it('checks a task when its box is clicked', async () => {
    const { root } = mount()
    type(root, '[] ferdig')
    await tick()
    const task = root.querySelector('[data-type="task"]') as HTMLElement
    task.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true, clientX: -1, button: 0 }))
    await tick()
    expect(root.querySelector('[data-type="task"]')?.getAttribute('data-checked')).toBe('true')
  })

  it('renders a pasted markdown draft with tables and images', async () => {
    const { root, editor } = mount()
    editor.insertMarkdown('## Tittel\n| a | b |\n| --- | --- |\n| 1 | 2 |\n![bilde](https://x.no/b.png)')
    await tick()
    expect(root.querySelector('[data-type="heading"][data-level="2"]')?.textContent).toBe('Tittel')
    expect(root.querySelector('pre[data-type="table"] [data-head]')?.textContent).toBe('| a | b |')
    expect(root.querySelector('a[data-image]')?.getAttribute('href')).toBe('https://x.no/b.png')
  })
})

describe('MarkdownInput toolbar commands', () => {
  it('applies marks to the selection', async () => {
    const { root, editor } = mount()
    type(root, 'hei')
    selectAll(root)
    editor.format('bold')
    await tick()
    expect(root.querySelector('strong')?.textContent).toBe('hei')
    selectAll(root)
    editor.format('underline')
    await tick()
    expect(root.querySelector('u')?.textContent).toBe('hei')
  })

  it('turns the current line into blocks and back', async () => {
    const { root, editor } = mount()
    type(root, 'linje')
    editor.block('ordered')
    await tick()
    expect(root.querySelector('[data-type="ordered"]')?.getAttribute('data-n')).toBe('1')
    editor.block('ordered')
    await tick()
    expect(root.querySelector('[data-type="paragraph"]')?.textContent).toBe('linje')
    editor.block('heading', { level: 3 })
    await tick()
    expect(root.querySelector('[data-type="heading"][data-level="3"]')).not.toBeNull()
    editor.block('code')
    await tick()
    expect(root.querySelector('pre[data-type="code"]')?.textContent).toBe('linje')
  })

  it('inserts a table and a divider below the line', async () => {
    const { root, editor } = mount()
    type(root, 'over')
    editor.block('table')
    await tick()
    expect(root.querySelector('pre[data-type="table"]')).not.toBeNull()
    editor.block('rule')
    await tick()
    expect(root.querySelector('[data-type="rule"]')).not.toBeNull()
  })

  it('inserts a mention as a mention link', async () => {
    const { root, editor } = mount()
    type(root, 'hei @ka')
    editor.insertMention(3, 'u1', 'Kari Nordmann')
    await tick()
    const mention = root.querySelector('a[data-mention]')
    expect(mention?.getAttribute('href')).toBe('mention:u1')
    expect(mention?.textContent).toBe('@Kari Nordmann')
  })
})

describe('MarkdownInput markdown source switch', () => {
  type Editor = { toggleSource: () => void; isSource: () => boolean; insertMarkdown: (t: string) => void }

  function mountSource() {
    const result = render(MarkdownInput, { props: { submitOnEnter: false } })
    const root = result.container.querySelector('[data-composer]') as HTMLDivElement
    root.focus()
    return { container: result.container, root, editor: result.component as unknown as Editor }
  }

  it('shows the switch only while there is markdown', async () => {
    const { container, root } = mountSource()
    type(root, 'bare tekst')
    await tick()
    expect(container.querySelector('button[aria-label="Markdown source"]')).toBeNull()
    type(root, ' **fet**')
    await tick()
    expect(container.querySelector('button[aria-label="Markdown source"]')).not.toBeNull()
  })

  it('toggles to the exact source and back without changing the content', async () => {
    const { container, root, editor } = mountSource()
    editor.insertMarkdown('## Plan\n- [x] gjort\n| a | b |\n| --- | --- |\n| 1 | 2 |')
    await tick()
    const before = root.innerHTML
    editor.toggleSource()
    await tick()
    const area = container.querySelector('textarea') as HTMLTextAreaElement
    expect(area.value).toBe('## Plan\n- [x] gjort\n| a | b |\n| --- | --- |\n| 1 | 2 |')
    expect(root.classList.contains('hidden')).toBe(true)
    editor.toggleSource()
    await tick()
    expect(container.querySelector('textarea')).toBeNull()
    expect(root.innerHTML).toBe(before)
  })

  it('maps the caret into the source', async () => {
    const { container, root, editor } = mountSource()
    type(root, '# Plan')
    await tick()
    editor.toggleSource()
    await tick()
    const area = container.querySelector('textarea') as HTMLTextAreaElement
    expect(area.selectionStart).toBe('# Plan'.length)
  })

  it('re-parses an edited source and keeps the undo history', async () => {
    const { container, root, editor } = mountSource()
    type(root, 'hei **du**')
    await tick()
    editor.toggleSource()
    await tick()
    const area = container.querySelector('textarea') as HTMLTextAreaElement
    area.value = '# hei **du**'
    area.dispatchEvent(new Event('input', { bubbles: true }))
    editor.toggleSource()
    await tick()
    expect(root.querySelector('[data-type="heading"] strong')?.textContent).toBe('du')
    key(root, { key: 'z', metaKey: true })
    await tick()
    expect(root.querySelector('[data-type="heading"]')).toBeNull()
    expect(root.querySelector('[data-type="paragraph"] strong')?.textContent).toBe('du')
  })

  it('converts pasted markdown text and keeps plain text literal', async () => {
    const { root } = mountSource()
    const paste = (text: string) => {
      const event = new Event('paste', { bubbles: true, cancelable: true }) as ClipboardEvent
      Object.defineProperty(event, 'clipboardData', { value: { files: [], getData: (kind: string) => (kind === 'text/plain' ? text : '') } })
      root.dispatchEvent(event)
    }
    paste('## Tittel\n- punkt')
    await tick()
    expect(root.querySelector('[data-type="heading"]')?.textContent).toBe('Tittel')
    expect(root.querySelector('[data-type="bullet"]')?.textContent).toBe('punkt')
  })
})

describe('MarkdownInput Enter rules', () => {
  function mountSend(props: Record<string, unknown> = {}) {
    const sent: boolean[] = []
    const result = render(MarkdownInput, { props: { submitOnEnter: true, onsubmit: (done: boolean) => sent.push(done), ...props } })
    const root = result.container.querySelector('[data-composer]') as HTMLDivElement
    root.focus()
    return { root, sent, editor: result.component as unknown as { block: (k: string) => void } }
  }

  it('sends on plain Enter and on ⌘ Enter, and marks done with ⌘ ⇧ Enter', async () => {
    const { root, sent } = mountSend()
    type(root, 'hei')
    key(root, { key: 'Enter' })
    key(root, { key: 'Enter', metaKey: true })
    key(root, { key: 'Enter', metaKey: true, shiftKey: true })
    await tick()
    expect(sent).toEqual([false, false, true])
    expect(root.querySelectorAll('[data-type="paragraph"]').length).toBe(1)
  })

  it('makes a new line on Shift Enter', async () => {
    const { root, sent } = mountSend()
    type(root, 'første')
    key(root, { key: 'Enter', shiftKey: true })
    type(root, 'andre')
    await tick()
    expect(sent).toEqual([])
    expect(root.querySelectorAll('[data-type="paragraph"]').length).toBe(2)
  })

  it('keeps editing in a code block and sends from it with ⌘ Enter', async () => {
    const { root, sent, editor } = mountSend()
    type(root, 'let a = 1')
    editor.block('code')
    key(root, { key: 'Enter' })
    type(root, 'let b = 2')
    await tick()
    expect(sent).toEqual([])
    expect(root.querySelector('pre[data-type="code"]')?.textContent).toContain('let b = 2')
    key(root, { key: 'Enter', metaKey: true })
    expect(sent).toEqual([false])
  })

  it('adds a list item on Enter instead of sending', async () => {
    const { root, sent } = mountSend()
    type(root, '- melk')
    key(root, { key: 'Enter' })
    type(root, 'brød')
    await tick()
    expect(sent).toEqual([])
    expect([...root.querySelectorAll('[data-type="bullet"]')].map((b) => b.textContent)).toEqual(['melk', 'brød'])
  })

  it('adds a table row on Enter instead of sending', async () => {
    const { root, sent, editor } = mountSend()
    editor.block('table')
    await tick()
    key(root, { key: 'Enter' })
    await tick()
    expect(sent).toEqual([])
    expect(root.querySelector('pre[data-type="table"]')).not.toBeNull()
  })

  it('leaves Enter to an open suggestion menu', async () => {
    const { root, sent } = mountSend({ onkeydown: (e: KeyboardEvent) => e.key === 'Enter' && e.preventDefault() })
    type(root, 'hei @ka')
    key(root, { key: 'Enter' })
    await tick()
    expect(sent).toEqual([])
  })

  it('never sends while an IME composition is running', async () => {
    const { root, sent } = mountSend()
    type(root, 'kan')
    root.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }))
    key(root, { key: 'Enter' })
    root.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }))
    key(root, { key: 'Enter', isComposing: true })
    key(root, { key: 'Enter', keyCode: 229 })
    await tick()
    expect(sent).toEqual([])
  })

  it('makes a new line on Enter and sends with ⌘ Enter when the setting is ⌘ Enter', async () => {
    const { root, sent } = mountSend({ submitOnEnter: false })
    type(root, 'første')
    key(root, { key: 'Enter' })
    type(root, 'andre')
    await tick()
    expect(sent).toEqual([])
    expect(root.querySelectorAll('[data-type="paragraph"]').length).toBe(2)
    key(root, { key: 'Enter', metaKey: true })
    expect(sent).toEqual([false])
  })
})

describe('MarkdownInput emoji shortcodes', () => {
  it('shows a typed shortcode as the emoji while the document keeps the shortcode', async () => {
    const docs: string[] = []
    const result = render(MarkdownInput, { props: { submitOnEnter: false, onshortcode: () => {} } })
    const root = result.container.querySelector('[data-composer]') as HTMLDivElement
    root.focus()
    type(root, 'Takk :tada: nå')
    await tick()
    const span = root.querySelector('[data-emoji=":tada:"]')
    expect(span?.textContent).toBe('🎉')
    expect(span?.getAttribute('contenteditable')).toBe('false')
    const { fromEditorDom, toMarkdown } = await import('./markdown')
    docs.push(toMarkdown(fromEditorDom(root)))
    expect(docs[0]).toBe('Takk :tada: nå')
  })

  it('reports the shortcode being typed and replaces it with the picked one', async () => {
    const queries: (string | null)[] = []
    const result = render(MarkdownInput, { props: { submitOnEnter: false, onshortcode: (q: string | null) => queries.push(q) } })
    const root = result.container.querySelector('[data-composer]') as HTMLDivElement
    root.focus()
    type(root, 'ship :ro')
    await tick()
    expect(queries.at(-1)).toBe('ro')
    ;(result.component as unknown as { insertShortcode: (l: number, c: string) => void }).insertShortcode(3, ':rocket:')
    await tick()
    expect(root.querySelector('[data-emoji=":rocket:"]')?.textContent).toBe('🚀')
    const { fromEditorDom, toMarkdown } = await import('./markdown')
    expect(toMarkdown(fromEditorDom(root))).toBe('ship :rocket: ')
  })
})
