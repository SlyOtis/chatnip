import { describe, expect, it } from 'vitest'
import {
  enterAction,
  applyRules,
  backspaceAtStart,
  blockText,
  docText,
  emptyDoc,
  enter,
  exitCode,
  fromEditorDom,
  fromMarkdown,
  htmlToDoc,
  insertDoc,
  insertText,
  run,
  textToDoc,
  toEditorHtml,
  toHtml,
  toMarkdown,
  toggleMark,
  toggleTask,
  indent,
  insertBlock,
  setBlocks,
  blockOf,
  mentionRun,
  splitBlock,
  TABLE_TEMPLATE,
  hasMarkdown,
  looksLikeMarkdown,
  markdownMap,
  sourceOffset,
  visibleOffset,
  type Doc
} from './markdown'

function typeAll(text: string, start: Doc = emptyDoc()): { doc: Doc; pos: number } {
  let doc = start
  let pos = docText(doc).length
  for (const ch of text) {
    const r = insertText(doc, pos, ch)
    const ruled = applyRules(r.doc, r.pos)
    doc = ruled.doc
    pos = ruled.pos
  }
  return { doc, pos }
}

describe('input rules', () => {
  it('turns **x** into bold', () => {
    const { doc, pos } = typeAll('hi **there**')
    expect(doc[0].runs).toEqual([run('hi '), run('there', ['bold'])])
    expect(pos).toBe(8)
  })

  it('turns *x* and _x_ into italic', () => {
    expect(typeAll('*a*').doc[0].runs).toEqual([run('a', ['italic'])])
    expect(typeAll('_b_').doc[0].runs).toEqual([run('b', ['italic'])])
  })

  it('leaves snake_case alone', () => {
    expect(blockText(typeAll('snake_case_name').doc[0])).toBe('snake_case_name')
  })

  it('turns ~x~ and ~~x~~ into strike', () => {
    expect(typeAll('~a~').doc[0].runs).toEqual([run('a', ['strike'])])
    expect(typeAll('~~b~~').doc[0].runs).toEqual([run('b', ['strike'])])
  })

  it('turns `x` into inline code', () => {
    expect(typeAll('run `ls -la`').doc[0].runs).toEqual([run('run '), run('ls -la', ['code'])])
  })

  it('does not apply marks inside inline code', () => {
    const { doc } = typeAll('`a*b*`')
    expect(doc[0].runs).toEqual([run('a*b*', ['code'])])
  })

  it('turns block prefixes into block types', () => {
    expect(typeAll('> q').doc[0]).toEqual({ type: 'quote', runs: [run('q')] })
    expect(typeAll('- a').doc[0]).toEqual({ type: 'bullet', runs: [run('a')] })
    expect(typeAll('* a').doc[0]).toEqual({ type: 'bullet', runs: [run('a')] })
    expect(typeAll('1. a').doc[0]).toEqual({ type: 'ordered', runs: [run('a')] })
  })

  it('turns [text](url) into a link', () => {
    const { doc } = typeAll('see [docs](https://x.no/a)')
    expect(doc[0].runs).toEqual([run('see '), run('docs', [], 'https://x.no/a')])
  })

  it('links a typed URL when followed by a space', () => {
    const { doc } = typeAll('go https://x.no/a. ')
    expect(doc[0].runs).toEqual([run('go '), run('https://x.no/a', [], 'https://x.no/a'), run('. ')])
  })

  it('turns ``` + Enter into a code block and Enter inside adds newlines', () => {
    let { doc, pos } = typeAll('```ts')
    ;({ doc, pos } = enter(doc, pos))
    expect(doc[0]).toEqual({ type: 'code', runs: [], lang: 'ts' })
    ;({ doc, pos } = typeAll('a*b*', doc))
    ;({ doc, pos } = enter(doc, pos))
    ;({ doc, pos } = typeAll('c', doc))
    expect(doc).toHaveLength(1)
    expect(blockText(doc[0])).toBe('a*b*\nc')
    expect(pos).toBe(6)
  })
})

describe('block exits', () => {
  it('exits a code block when ``` is typed on its own line', () => {
    let { doc, pos } = typeAll('```')
    ;({ doc, pos } = enter(doc, pos))
    ;({ doc, pos } = typeAll('x', doc))
    ;({ doc, pos } = enter(doc, pos))
    ;({ doc, pos } = typeAll('```', doc))
    ;({ doc, pos } = enter(doc, pos))
    expect(doc.map((b) => b.type)).toEqual(['code', 'paragraph'])
    expect(blockText(doc[0])).toBe('x')
    expect(pos).toBe(2)
  })

  it('exits a code block with ArrowDown at its end', () => {
    const doc: Doc = [{ type: 'code', runs: [run('x')] }]
    expect(exitCode(doc, 0)).toBeNull()
    const r = exitCode(doc, 1)!
    expect(r.doc.map((b) => b.type)).toEqual(['code', 'paragraph'])
    expect(r.pos).toBe(2)
  })

  it('Enter on an empty list item leaves the list', () => {
    const doc: Doc = [{ type: 'bullet', runs: [run('a')] }, { type: 'bullet', runs: [] }]
    const r = enter(doc, 2)
    expect(r.doc[1].type).toBe('paragraph')
  })

  it('Enter in a list item continues the list', () => {
    const r = enter([{ type: 'ordered', runs: [run('ab')] }], 1)
    expect(r.doc).toEqual([{ type: 'ordered', runs: [run('a')] }, { type: 'ordered', runs: [run('b')] }])
    expect(r.pos).toBe(2)
  })

  it('Backspace at block start removes the block type, then merges', () => {
    const doc: Doc = [{ type: 'paragraph', runs: [run('a')] }, { type: 'quote', runs: [run('b')] }]
    const once = backspaceAtStart(doc, 2)!
    expect(once.doc[1].type).toBe('paragraph')
    const twice = backspaceAtStart(once.doc, once.pos)!
    expect(twice.doc).toEqual([{ type: 'paragraph', runs: [run('ab')] }])
    expect(twice.pos).toBe(1)
    expect(backspaceAtStart(twice.doc, 0)).toBeNull()
  })
})

describe('marks', () => {
  it('toggles a mark over a range', () => {
    const doc = fromMarkdown('hello world')
    const bold = toggleMark(doc, 0, 5, 'bold')
    expect(toMarkdown(bold)).toBe('**hello** world')
    expect(toMarkdown(toggleMark(bold, 0, 5, 'bold'))).toBe('hello world')
  })

  it('serializes nested marks', () => {
    const doc: Doc = [{ type: 'paragraph', runs: [run('a', ['bold']), run('b', ['bold', 'italic']), run('c', ['italic'])] }]
    expect(toMarkdown(doc)).toBe('**a_b_**_c_')
    expect(fromMarkdown('**a _b_** _c_')[0].runs).toEqual([run('a ', ['bold']), run('b', ['bold', 'italic']), run(' '), run('c', ['italic'])])
  })

  it('keeps code innermost', () => {
    const doc: Doc = [{ type: 'paragraph', runs: [run('x', ['code']), run('y', ['bold', 'code'])] }]
    expect(toMarkdown(doc)).toBe('`x`**`y`**')
  })
})

describe('round trips', () => {
  const cases = [
    'plain text',
    '**bold** and _italic_ and ~strike~',
    'nested **bold _italic_ bold**',
    '`code` with **bold**',
    '> a quote',
    '- one\n- two',
    '1. one\n2. two',
    '```ts\nconst a = 1\n\nlet b\n```',
    'see https://chatnip.app/x',
    '[docs](https://x.no/a) here',
    '[**bold link**](https://x.no)',
    'line one\n\nline three',
    '@Kari Nordmann hi'
  ]
  for (const md of cases) {
    it(md, () => {
      expect(toMarkdown(fromMarkdown(md))).toBe(md)
    })
  }

  it('renumbers ordered lists', () => {
    expect(toMarkdown(fromMarkdown('3. a\n7. b'))).toBe('1. a\n2. b')
  })

  it('round trips through the editor DOM', () => {
    const doc = fromMarkdown('**a** _b_ [c](https://x.no)\n> q\n```\nx\n\n```\n- l\n\n1. o')
    const root = document.createElement('div')
    root.innerHTML = toEditorHtml(doc)
    expect(fromEditorDom(root)).toEqual(doc)
  })
})

describe('html', () => {
  it('renders the model to html', () => {
    const doc = fromMarkdown('**a** <b>\n- x\n- y\n```\n<p>\n```\n[l](https://x.no?a=1&b=2)')
    const html = toHtml(doc)
    expect(html.startsWith('<p><strong>a</strong> &lt;b&gt;</p><ul class="msg-list"><li>x</li><li>y</li></ul><div class="code-block-wrapper">')).toBe(true)
    expect(html).toContain('<pre class="code-block"><code>&lt;p&gt;</code></pre>')
    expect(html.endsWith('<p><a href="https://x.no?a=1&amp;b=2" target="_blank" rel="noopener noreferrer" class="msg-link">l</a></p>')).toBe(true)
  })

  it('converts pasted html', () => {
    const doc = htmlToDoc('<meta charset="utf-8"><p>Hi <b>bold</b> <em>it</em> <a href="https://x.no">link</a> <code>c</code></p><ul><li>one</li><li>two</li></ul><ol><li>first</li></ol><pre>a\n b\n</pre><blockquote>q</blockquote>')
    expect(toMarkdown(doc)).toBe('Hi **bold** _it_ [link](https://x.no) `c`\n- one\n- two\n1. first\n```\na\n b\n```\n> q')
  })

  it('reads span styles and ignores Google Docs wrappers', () => {
    const doc = htmlToDoc('<b style="font-weight:normal" id="docs-internal-guid-1"><span style="font-weight:700">B</span> <span style="font-style:italic">I</span> <span style="text-decoration:line-through">S</span></b>')
    expect(toMarkdown(doc)).toBe('**B** _I_ ~S~')
  })

  it('turns br into new lines and autolinks bare URLs', () => {
    expect(toMarkdown(htmlToDoc('a<br>https://x.no b'))).toBe('a\nhttps://x.no b')
  })
})

describe('paste', () => {
  it('pastes plain text lines with links', () => {
    const r = insertDoc(fromMarkdown('ab'), 1, textToDoc('x https://x.no\ny'))
    expect(toMarkdown(r.doc)).toBe('ax https://x.no\nyb')
    expect(r.pos).toBe(17)
  })

  it('pastes into a code block as raw text', () => {
    const r = insertDoc([{ type: 'code', runs: [run('ab')] }], 1, fromMarkdown('**x**\ny'))
    expect(blockText(r.doc[0])).toBe('ax\nyb')
  })
})

describe('full markdown input rules', () => {
  it('turns # through ###### into headings', () => {
    expect(typeAll('# Plan').doc[0]).toEqual({ type: 'heading', level: 1, runs: [run('Plan')] })
    expect(typeAll('###### Detalj').doc[0]).toEqual({ type: 'heading', level: 6, runs: [run('Detalj')] })
    expect(typeAll('#hashtag').doc[0]).toEqual({ type: 'paragraph', runs: [run('#hashtag')] })
  })

  it('turns - [ ] and [x] into task items', () => {
    expect(typeAll('- [ ] skriv').doc[0]).toEqual({ type: 'task', checked: false, runs: [run('skriv')] })
    expect(typeAll('[x] ferdig').doc[0]).toEqual({ type: 'task', checked: true, runs: [run('ferdig')] })
  })

  it('turns ![alt](url) into an image and <url> into a link', () => {
    const image = typeAll('se ![diagram](https://x.no/d.png)').doc[0]
    expect(image.runs).toEqual([run('se '), run('diagram', ['image'], 'https://x.no/d.png')])
    const auto = typeAll('<https://x.no/a>').doc[0]
    expect(auto.runs).toEqual([run('https://x.no/a', [], 'https://x.no/a')])
  })

  it('turns --- + Enter into a divider and a header row + Enter into a table', () => {
    let { doc, pos } = typeAll('---')
    ;({ doc, pos } = enter(doc, pos))
    expect(doc.map((b) => b.type)).toEqual(['rule', 'paragraph'])
    expect(pos).toBe(1)
    ;({ doc, pos } = typeAll('| Navn | Antall |'))
    ;({ doc, pos } = enter(doc, pos))
    expect(doc[0].type).toBe('table')
    expect(blockText(doc[0])).toBe('| Navn | Antall |\n| --- | --- |\n| ')
    ;({ doc, pos } = typeAll('Kari | 3 |', doc))
    ;({ doc, pos } = enter(doc, pos))
    ;({ doc, pos } = enter(doc, pos))
    expect(doc.map((b) => b.type)).toEqual(['table', 'paragraph'])
    expect(blockText(doc[0])).toBe('| Navn | Antall |\n| --- | --- |\n| Kari | 3 |')
  })

  it('nests and unnests list items with indent and Backspace', () => {
    const doc: Doc = [blockOf('bullet', [run('a')]), blockOf('bullet', [run('b')])]
    const nested = indent(doc, 2, 2, 1)!
    expect(nested[1]).toEqual({ type: 'bullet', depth: 1, runs: [run('b')] })
    expect(indent(nested, 2, 2, 1)![1].depth).toBe(1)
    expect(indent(doc, 0, 0, 1)![0].depth).toBeUndefined()
    expect(backspaceAtStart(nested, 2)!.doc[1]).toEqual({ type: 'bullet', runs: [run('b')] })
    expect(indent([blockOf('paragraph', [run('x')])], 0, 0, 1)).toBeNull()
  })

  it('Enter on an empty nested item outdents before it leaves the list', () => {
    const doc: Doc = [blockOf('bullet', [run('a')]), blockOf('bullet', [], { depth: 1 })]
    const once = splitBlock(doc, 2)
    expect(once.doc[1]).toEqual({ type: 'bullet', runs: [] })
    expect(splitBlock(once.doc, 2).doc[1].type).toBe('paragraph')
  })

  it('Enter at the end of a heading continues with a paragraph', () => {
    const r = enter([blockOf('heading', [run('Tittel')], { level: 2 })], 6)
    expect(r.doc.map((b) => b.type)).toEqual(['heading', 'paragraph'])
  })

  it('toggles a task and removes a divider with Backspace', () => {
    expect(toggleTask([blockOf('task', [run('a')])], 0)[0].checked).toBe(true)
    const r = backspaceAtStart([blockOf('paragraph', [run('a')]), blockOf('rule', []), blockOf('paragraph', [run('b')])], 3)!
    expect(r.doc.map((b) => b.type)).toEqual(['paragraph', 'paragraph'])
    expect(r.pos).toBe(2)
  })
})

describe('toolbar commands', () => {
  it('sets and clears a block type over the selected blocks', () => {
    const doc = fromMarkdown('en\nto')
    const quoted = setBlocks(doc, 0, 4, 'quote')
    expect(toMarkdown(quoted)).toBe('> en\n> to')
    expect(toMarkdown(setBlocks(quoted, 0, 4, 'quote'))).toBe('en\nto')
    expect(toMarkdown(setBlocks(doc, 0, 0, 'ordered'))).toBe('1. en\nto')
    expect(toMarkdown(setBlocks(doc, 0, 0, 'heading', { level: 2 }))).toBe('## en\nto')
    expect(toMarkdown(setBlocks(doc, 0, 0, 'task'))).toBe('- [ ] en\nto')
  })

  it('wraps selected lines in a code block and unwraps it again', () => {
    const doc = fromMarkdown('a\nb\nc')
    const code = setBlocks(doc, 0, 3, 'code')
    expect(toMarkdown(code)).toBe('```\na\nb\n```\nc')
    expect(toMarkdown(setBlocks(code, 0, 0, 'code'))).toBe('a\nb\nc')
  })

  it('inserts a table and a divider', () => {
    const table = insertBlock(emptyDoc(), 0, blockOf('table', [run(TABLE_TEMPLATE)]), 2)
    expect(table.doc).toEqual([{ type: 'table', runs: [run(TABLE_TEMPLATE)] }])
    expect(table.pos).toBe(2)
    const rule = insertBlock(fromMarkdown('tekst'), 2, blockOf('rule', []))
    expect(rule.doc.map((b) => b.type)).toEqual(['paragraph', 'rule', 'paragraph'])
    expect(rule.pos).toBe(7)
  })

  it('toggles underline as <u>', () => {
    expect(toMarkdown(toggleMark(fromMarkdown('hei du'), 0, 3, 'underline'))).toBe('<u>hei</u> du')
    expect(fromMarkdown('<u>hei</u> du')[0].runs).toEqual([run('hei', ['underline']), run(' du')])
  })
})

describe('full markdown round trips', () => {
  const cases = [
    '# Plan\n## Uke **42**\n###### Detalj',
    '- topp\n  - under\n    - dypest\n- tilbake',
    '1. en\n  1. en.en\n  2. en.to\n2. to',
    '- [ ] skriv\n- [x] test',
    '| Navn | Antall |\n| :--- | ---: |\n| Kari | **3** |',
    'over\n---\nunder',
    'se ![diagram](https://x.no/d.png) her',
    'hei [@Kari Nordmann](mention:u1) og <u>du</u>',
    '~~~\nfence\n~~~'.replace(/~~~/g, '```'),
    '> sitat med `kode`'
  ]
  for (const md of cases) {
    it(md, () => {
      expect(toMarkdown(fromMarkdown(md))).toBe(md)
    })
  }

  it('reads tilde fences, star rules, autolinks and escapes', () => {
    expect(toMarkdown(fromMarkdown('~~~ts\nx\n~~~'))).toBe('```ts\nx\n```')
    expect(fromMarkdown('***')[0].type).toBe('rule')
    expect(fromMarkdown('<https://x.no>')[0].runs).toEqual([run('https://x.no', [], 'https://x.no')])
    expect(fromMarkdown('\\*ikke\\*')[0].runs).toEqual([run('*ikke*')])
    expect(fromMarkdown('[x](javascript:alert(1))')[0].runs.every((r) => !r.href)).toBe(true)
    expect(fromMarkdown('kari@x.no og www.x.no')[0].runs).toEqual([run('kari@x.no og www.x.no')])
  })

  it('serializes a mention run as a mention link', () => {
    expect(toMarkdown([blockOf('paragraph', [run('hei '), mentionRun('u1', 'Kari')])])).toBe('hei [@Kari](mention:u1)')
  })

  it('round trips every block through the editor DOM', () => {
    const doc = fromMarkdown('# T\n- a\n  - [x] b\n1. c\n---\n| a | b |\n| --- | --- |\n| 1 | 2 |\n![i](https://x.no/i.png) [@K](mention:u1) <u>u</u>\n```rs\nfn x()\n```')
    const root = document.createElement('div')
    root.innerHTML = toEditorHtml(doc)
    expect(fromEditorDom(root)).toEqual(doc)
    expect(root.querySelector('[data-type="heading"]')?.getAttribute('data-level')).toBe('1')
    expect(root.querySelector('[data-type="task"]')?.getAttribute('data-checked')).toBe('true')
    expect(root.querySelector('[data-type="ordered"]')?.getAttribute('data-n')).toBe('1')
    expect(root.querySelectorAll('pre[data-type="table"] [data-pipe]').length).toBeGreaterThan(3)
    expect(root.querySelector('a[data-image]')?.textContent).toBe('i')
    expect(root.querySelector('a[data-mention]')?.textContent).toBe('@K')
  })
})

describe('timeline html', () => {
  const dom = (html: string) => {
    const root = document.createElement('div')
    root.innerHTML = html
    return root
  }

  it('renders headings, nested lists, tasks, tables, rules and images', () => {
    const root = dom(toHtml(fromMarkdown('## Plan\n- a\n  - b\n- [x] c\n1. d\n| A | B |\n| --- | :-: |\n| 1 | **2** |\n\n---\n![i](https://x.no/i.png)')))
    expect(root.querySelector('h2')?.textContent).toBe('Plan')
    expect(root.querySelector('ul li ul li')?.textContent).toBe('b')
    expect((root.querySelector('.msg-task input') as HTMLInputElement).checked).toBe(true)
    expect(root.querySelector('ol li')?.textContent).toBe('d')
    expect([...root.querySelectorAll('th')].map((n) => n.textContent)).toEqual(['A', 'B'])
    expect(root.querySelector('td strong')?.textContent).toBe('2')
    expect((root.querySelectorAll('td')[1] as HTMLElement).style.textAlign).toBe('center')
    expect(root.querySelector('hr')).not.toBeNull()
    expect(root.querySelector('img')?.getAttribute('src')).toBe('https://x.no/i.png')
  })

  it('renders mentions as names, keeps lines as breaks and escapes html', () => {
    const root = dom(toHtml(fromMarkdown('hei [@Kari](mention:u1)\nlinje to\n<img src=x onerror=alert(1)>')))
    expect(root.querySelector('.msg-mention')?.textContent).toBe('@Kari')
    expect(root.querySelectorAll('br').length).toBe(2)
    expect(root.querySelector('img')).toBeNull()
  })

  it('never links an unsafe href written in the editor', () => {
    const root = dom(toHtml([blockOf('paragraph', [run('x', [], 'javascript:alert(1)')])]))
    expect(root.querySelector('a')).toBeNull()
  })
})

describe('markdown detection', () => {
  it('detects formatting, blocks, labelled links and images but not plain text, bare URLs or mentions', () => {
    expect(hasMarkdown(fromMarkdown('bare tekst og https://x.no'))).toBe(false)
    expect(hasMarkdown([blockOf('paragraph', [run('hei '), mentionRun('u1', 'Kari')])])).toBe(false)
    expect(hasMarkdown(fromMarkdown('litt **fet**'))).toBe(true)
    expect(hasMarkdown(fromMarkdown('# Tittel'))).toBe(true)
    expect(hasMarkdown(fromMarkdown('[lenke](https://x.no)'))).toBe(true)
    expect(hasMarkdown(fromMarkdown('![b](https://x.no/b.png)'))).toBe(true)
    expect(looksLikeMarkdown('- en\n- to')).toBe(true)
    expect(looksLikeMarkdown('bare en setning, 2 * 3')).toBe(false)
  })
})

describe('literal text stays literal', () => {
  it('escapes markdown syntax only where it would otherwise format', () => {
    const literal = textToDoc('**ikke fet** og snake_case\n# ikke tittel\n- ikke liste\n1. ikke tall')
    const md = toMarkdown(literal)
    expect(md).toBe('\\*\\*ikke fet\\*\\* og snake\\_case\n\\# ikke tittel\n\\- ikke liste\n1\\. ikke tall')
    expect(fromMarkdown(md)).toEqual(literal)
    expect(toMarkdown(textToDoc('snake_case og 2 * 3'))).toBe('snake_case og 2 * 3')
  })
})

describe('source map', () => {
  it('maps the caret into the markdown source and back', () => {
    const doc = fromMarkdown('## Plan **nå**\n- [x] gjort\nse [docs](https://x.no) her')
    const map = markdownMap(doc)
    expect(map.text).toBe(toMarkdown(doc))
    expect(map.map).toHaveLength(docText(doc).length + 1)
    expect(sourceOffset(map, 0)).toBe(3)
    expect(map.text.slice(sourceOffset(map, 5))).toMatch(/^nå\*\*/)
    expect(map.text.slice(sourceOffset(map, 6))).toMatch(/^å/)
    expect(map.text.slice(sourceOffset(map, 7))).toMatch(/^\*\*\n/)
    const task = docText(doc).indexOf('gjort')
    expect(map.text.slice(sourceOffset(map, task))).toMatch(/^gjort/)
    const docs = docText(doc).indexOf('docs')
    expect(map.text.slice(sourceOffset(map, docs))).toMatch(/^docs\]/)
    for (const pos of [0, 5, 7, task, docs, docText(doc).length]) expect(visibleOffset(map, sourceOffset(map, pos))).toBe(pos)
  })

  it('round trips the source exactly', () => {
    for (const md of ['# T\n- a\n  - b\n1. c\n| a | b |\n| --- | --- |\n| 1 | 2 |\n---\n```ts\nx\n```', 'hei [@Kari](mention:u1) <u>u</u> ~s~ `c`']) {
      expect(markdownMap(fromMarkdown(md)).text).toBe(md)
    }
  })
})

describe('enterAction', () => {
  const plain = { shiftKey: false, metaKey: false, ctrlKey: false }
  const block = (type: Parameters<typeof blockOf>[0]) => blockOf(type, [run('x')])

  it('sends on Enter only in plain blocks while Enter sends', () => {
    expect(enterAction(block('paragraph'), plain, true)).toBe('send')
    expect(enterAction(block('heading'), plain, true)).toBe('send')
    expect(enterAction(block('quote'), plain, true)).toBe('send')
    for (const type of ['code', 'table', 'bullet', 'ordered', 'task'] as const) expect(enterAction(block(type), plain, true)).toBe('edit')
    expect(enterAction(block('paragraph'), plain, false)).toBe('edit')
    expect(enterAction(block('paragraph'), { ...plain, shiftKey: true }, true)).toBe('edit')
    for (const line of ['```', '```ts', '---', '| name | qty |']) expect(enterAction(blockOf('paragraph', [run(line)]), plain, true)).toBe('edit')
    expect(enterAction(blockOf('paragraph', [run('a | b')]), plain, true)).toBe('send')
  })

  it('sends with ⌘ or Ctrl Enter anywhere and marks done with Shift', () => {
    expect(enterAction(block('code'), { ...plain, metaKey: true }, true)).toBe('send')
    expect(enterAction(block('bullet'), { ...plain, ctrlKey: true }, false)).toBe('send')
    expect(enterAction(block('paragraph'), { ...plain, metaKey: true, shiftKey: true }, true)).toBe('send-done')
  })

  it('leaves Enter to the input method while composing', () => {
    expect(enterAction(block('paragraph'), { ...plain, isComposing: true }, true)).toBe('compose')
    expect(enterAction(block('paragraph'), { ...plain, keyCode: 229 }, true)).toBe('compose')
    expect(enterAction(block('paragraph'), { ...plain, metaKey: true }, true, true)).toBe('compose')
  })
})
