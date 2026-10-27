import { describe, it, expect } from 'vitest'
import { parseMarkdown, sanitizeHtml, splitQuoted, isEmojiOnly, escapeHtml, prepareMail, mailDocument } from './content'
import outlookReplyTemplate from '../../src-tauri/tests/fixtures/mock/outlook-reply.html?raw'
import outlookQuoteTemplate from '../../src-tauri/tests/fixtures/mock/outlook-quote.html?raw'
import gmailReplyTemplate from '../../src-tauri/tests/fixtures/mock/gmail-reply.html?raw'
import gmailQuoteTemplate from '../../src-tauri/tests/fixtures/mock/gmail-quote.html?raw'
import appleEml from '../../src-tauri/tests/fixtures/mock/apple-reply.eml?raw'
import plainEml from '../../src-tauri/tests/fixtures/mock/plain-wrapped.eml?raw'
import retailerEml from '../../src-tauri/tests/fixtures/mock/retailer.eml?raw'
import darkEml from '../../src-tauri/tests/fixtures/mock/dark-hostile.eml?raw'
import githubEml from '../../src-tauri/tests/fixtures/mock/github.eml?raw'
import * as md from '../test/fixtures/content/markdown'
import * as mail from '../test/fixtures/content/mail'
import * as chat from '../test/fixtures/content/chat'

function dom(html: string): HTMLDivElement {
  const div = document.createElement('div')
  div.innerHTML = html
  return div
}

function text(html: string): string {
  return dom(html).textContent ?? ''
}

describe('parseMarkdown structure', () => {
  const root = dom(parseMarkdown(md.allMarkdown))

  it('renders headings, task lists, a table and a rule', () => {
    const doc = dom(parseMarkdown(md.documentMarkdown))
    expect(doc.querySelector('h1.msg-heading')?.textContent).toBe('Sprintplan uke 43')
    expect([...doc.querySelectorAll('h2.msg-heading')].map((n) => n.textContent)).toEqual(['Mål', 'Fordeling'])
    expect([...doc.querySelectorAll('li.msg-task')].map((n) => n.getAttribute('data-done'))).toEqual(['true', 'false'])
    expect([...doc.querySelectorAll('table.msg-table th')].map((n) => n.textContent)).toEqual(['Oppgave', 'Eier', 'Status'])
    expect(doc.querySelectorAll('table.msg-table tbody tr').length).toBe(2)
    expect(doc.querySelector('hr.msg-rule')).not.toBeNull()
    expect(doc.textContent).not.toContain('|---')
  })

  it('renders bold, slack bold, italic and strike', () => {
    expect([...root.querySelectorAll('strong')].map((n) => n.textContent)).toEqual(['Agenda', 'fredag'])
    expect(root.querySelector('em')?.textContent).toBe('torsdag')
    expect(root.querySelector('del')?.textContent).toBe('onsdag')
  })

  it('renders inline code and a blockquote', () => {
    expect(root.querySelector('code.inline-code')?.textContent).toBe('Cargo.lock')
    expect(root.querySelector('blockquote.msg-blockquote')?.textContent).toBe('Husk å oppdatere Cargo.lock først')
  })

  it('splits bullet and numbered lists', () => {
    expect([...root.querySelectorAll('ul.msg-list > li')].map((n) => n.textContent)).toEqual(['gå gjennom PR', 'bestem leverandør'])
    expect([...root.querySelectorAll('ol.msg-list-ordered > li')].map((n) => n.textContent)).toEqual(['demo', 'retro'])
  })

  it('renders a fenced block with language, escaped content and a copy button', () => {
    const wrapper = root.querySelector('.code-block-wrapper')
    expect(wrapper?.querySelector('.code-lang')?.textContent).toBe('rust')
    expect(wrapper?.querySelector('.code-copy-btn')).not.toBeNull()
    const code = wrapper?.querySelector('pre.code-block code')
    expect(code?.textContent).toContain('println!("<hei>");')
    expect(code?.querySelector('script, hei')).toBeNull()
  })

  it('keeps newlines inside fenced code so copy keeps them', () => {
    const code = root.querySelector('pre.code-block code')
    expect(code?.querySelectorAll('br').length).toBe(0)
    expect(code?.textContent).toBe('fn main() {\n    println!("<hei>");\n}')
  })

  it('renders a fenced block without language with only the copy header', () => {
    const block = dom(parseMarkdown(md.fencedWithoutLanguage))
    expect(block.querySelector('.code-lang')).toBeNull()
    expect(block.querySelector('.code-copy-btn')).not.toBeNull()
    expect(block.querySelector('code')?.textContent).toBe('npm run dev && echo "ok"')
  })

  it('links bare urls without their trailing punctuation', () => {
    const links = [...root.querySelectorAll('a.msg-link')]
    expect(links.map((a) => a.getAttribute('href'))).toEqual(['https://northwind.example/agenda'])
    expect(root.textContent).toContain('https://northwind.example/agenda.')
    expect(links[0].getAttribute('target')).toBe('_blank')
    expect(links[0].getAttribute('rel')).toBe('noopener noreferrer')
  })

  it('wraps a mention up to the next space or punctuation', () => {
    expect(root.querySelector('.msg-mention')?.textContent).toBe('@Eva')
    expect(dom(parseMarkdown('@Sam, kan du se?')).querySelector('.msg-mention')?.textContent).toBe('@Sam')
    expect(dom(parseMarkdown(md.mentionAtEnd)).querySelector('.msg-mention')?.textContent).toBe('@Sam')
  })

  it('never treats an email address as a mention', () => {
    const root = dom(parseMarkdown(md.emailAddress))
    expect(root.querySelector('.msg-mention')).toBeNull()
    expect(root.textContent).toBe(md.emailAddress)
  })

  it('never links urls inside inline or fenced code', () => {
    const code = dom(parseMarkdown(md.urlInCode)).querySelector('code.inline-code')
    expect(code?.querySelector('a')).toBeNull()
    expect(code?.textContent).toBe('curl https://api.northwind.example/health')
    const fenced = dom(parseMarkdown('```\ncurl https://a.no\n```'))
    expect(fenced.querySelector('a')).toBeNull()
  })

  it('renders labelled links', () => {
    const a = dom(parseMarkdown('se [agendaen](https://northwind.example/agenda) nå')).querySelector('a.msg-link')
    expect(a?.getAttribute('href')).toBe('https://northwind.example/agenda')
    expect(a?.textContent).toBe('agendaen')
  })

  it('keeps balanced parentheses in urls and drops an unbalanced closing one', () => {
    const wiki = dom(parseMarkdown('https://en.wikipedia.org/wiki/Rust_(language)')).querySelector('a')
    expect(wiki?.getAttribute('href')).toBe('https://en.wikipedia.org/wiki/Rust_(language)')
    const wrapped = dom(parseMarkdown('(se https://a.no/x)')).querySelector('a')
    expect(wrapped?.getAttribute('href')).toBe('https://a.no/x')
  })

  it('escapes raw html outside code', () => {
    const root = dom(parseMarkdown('<img src=x onerror=alert(1)> hei'))
    expect(root.querySelector('img')).toBeNull()
    expect(root.textContent).toContain('<img')
  })

  it('renders lists with formatting inside items', () => {
    const root = dom(parseMarkdown('- **viktig**\n- [lenke](https://a.no)\n\n1. en\n2) to'))
    expect(root.querySelector('ul li strong')?.textContent).toBe('viktig')
    expect(root.querySelector('ul li a')?.getAttribute('href')).toBe('https://a.no')
    expect([...root.querySelectorAll('ol li')].map((n) => n.textContent)).toEqual(['en', 'to'])
  })

  it('does not italicise snake_case identifiers', () => {
    const root = dom(parseMarkdown(md.snakeCase))
    expect([...root.querySelectorAll('em')].map((n) => n.textContent)).toEqual(['real emphasis'])
    expect(root.textContent).toContain('some_variable_name')
  })

  it('returns an empty string for empty input', () => {
    expect(parseMarkdown('')).toBe('')
  })

  it('renders a 10 000 character, 200 line message quickly', () => {
    expect(md.longMessage.length).toBeGreaterThanOrEqual(10_000)
    const started = performance.now()
    const html = parseMarkdown(md.longMessage)
    expect(performance.now() - started).toBeLessThan(50)
    const root = dom(html)
    expect(root.querySelectorAll('a.msg-link').length).toBe(200)
    expect(root.querySelectorAll('br').length).toBe(199)
    expect(root.querySelector('ol')).toBeNull()
  })

  it('keeps right to left and mixed scripts intact', () => {
    expect(text(parseMarkdown(md.rtlMixed))).toBe(md.rtlMixed)
  })
})

describe('sanitizeHtml', () => {
  it('drops scripts and every on* handler but keeps content and cid images', () => {
    const root = dom(sanitizeHtml(mail.hostile))
    expect(root.querySelector('script')).toBeNull()
    expect([...root.querySelectorAll('*')].flatMap((el) => [...el.attributes].map((a) => a.name)).filter((n) => n.startsWith('on'))).toEqual([])
    expect(root.querySelector('img')?.getAttribute('src')).toBe('cid:logo@client.example')
    expect(root.querySelector('a')?.getAttribute('href')).toBe('https://ok.no')
    expect(root.querySelector('p')?.textContent).toBe('Hei')
  })

  it('keeps Teams mentions, hosted images and code blocks', () => {
    const root = dom(sanitizeHtml(chat.teamsHtml))
    expect([...root.querySelectorAll('at')].map((n) => n.textContent)).toEqual(['Nora Gale', 'Sam Hale'])
    expect(root.querySelector('img')?.getAttribute('src')).toContain('/hostedContents/aWQ9/$value')
    expect(root.querySelector('codeblock')?.getAttribute('class')).toBe('Bash')
  })

  it('keeps the newsletter structure including the hidden preheader and the tracking pixel', () => {
    const root = dom(sanitizeHtml(mail.newsletter))
    expect(root.querySelectorAll('table table td').length).toBe(2)
    expect((root.querySelector('div') as HTMLElement).style.display).toBe('none')
    expect(root.querySelector('img[width="1"][height="1"]')).not.toBeNull()
  })
})

describe('splitQuoted html', () => {
  const cases: [string, string, string, string][] = [
    ['gmail', mail.gmailReply, 'Takk Milo, vi tar tilbudet', 'Vedlagt ligger tilbudet'],
    ['outlook desktop', mail.outlookReply, 'Hi, your access request is approved.', 'could I get access to the staging cluster?'],
    ['outlook web', mail.outlookWebReply, 'Ok, booket.', 'Kan du booke rommet?'],
    ['apple', mail.appleReply, 'Kommer! Skal jeg ta med noe? 🥗', 'Lunsj fredag kl 12?'],
    ['thunderbird', mail.thunderbirdReply, 'Sounds good.', 'Security review Friday?'],
    ['yahoo', mail.yahooReply, 'Yes.', 'Ready?'],
    ['proton', mail.protonReply, 'Signed.', 'Sign here.']
  ]

  for (const [name, html, visible, quoted] of cases) {
    it(`splits a ${name} reply`, () => {
      const parts = splitQuoted(html, true)
      expect(text(parts.visible)).toContain(visible)
      expect(text(parts.visible)).not.toContain(quoted)
      expect(text(parts.quoted)).toContain(quoted)
    })
  }

  it('folds the gmail signature apart from the visible text', () => {
    const parts = splitQuoted(mail.gmailReply, true)
    expect(text(parts.signature)).toContain('CTO, Kunde AS')
    expect(text(parts.visible)).not.toContain('CTO, Kunde AS')
    expect(text(parts.visible)).toContain('Takk Milo')
  })

  it('keeps the outlook signature line on the visible side', () => {
    expect(text(splitQuoted(mail.outlookReply, true).visible)).toContain('Kari Nordmann | Platform lead | Tidewater')
  })

  it('returns the input untouched when nothing is quoted', () => {
    expect(splitQuoted(mail.newsletter, true)).toEqual({ visible: mail.newsletter, quoted: '', signature: '' })
    expect(splitQuoted('', true)).toEqual({ visible: '', quoted: '', signature: '' })
  })
})

describe('splitQuoted text', () => {
  it('splits on an english wrote header', () => {
    const parts = splitQuoted(mail.plainGmailReply, false)
    expect(parts.visible).toBe('Takk Milo, vi tar tilbudet. Kan vi signere torsdag?\n\nAnna')
    expect(parts.quoted.split('\n')[1]).toBe('> Hei Anna,')
  })

  it('splits on inline > quotes', () => {
    expect(splitQuoted(mail.plainInlineQuote, false)).toEqual({ visible: 'Ja.', quoted: '> sitat\n> mer', signature: '' })
  })

  it('splits at a norwegian skrev header', () => {
    const parts = splitQuoted(mail.plainNorwegianReply, false)
    expect(parts.visible).toBe('Takk Mia!')
    expect(parts.quoted.split('\n')).toEqual(['Den 8. okt. 2026 kl. 10:00 skrev Mia Sund <mia.sund@freelance.example>:', '> Her er bildene.'])
  })

  it('does not split when the message starts with a quote', () => {
    expect(splitQuoted(mail.plainStartsWithQuote, false).quoted).toBe('')
  })
})

describe('isEmojiOnly', () => {
  for (const sample of chat.emojiOnly) {
    it(`accepts ${sample}`, () => expect(isEmojiOnly(sample)).toBe(true))
  }
  for (const sample of chat.notEmojiOnly) {
    it(`rejects ${JSON.stringify(sample)}`, () => expect(isEmojiOnly(sample)).toBe(false))
  }
})

describe('escapeHtml', () => {
  it('escapes the four html specials', () => {
    expect(escapeHtml('<a href="x">&</a>')).toBe('&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;')
  })
})

function emlBody(raw: string): string {
  return raw.replace(/\r\n/g, '\n').split('\n\n').slice(1).join('\n\n')
}

function outlookThread(depth: number): string {
  let html = ''
  for (let i = 0; i < depth; i++) {
    const inner = html.split('<div class="WordSection1">')[1]?.replace(/<\/div>\s*<\/body><\/html>\s*$/, '') ?? ''
    const quote = i === 0 ? '' : outlookQuoteTemplate.replace('{name}', `Person ${i - 1}`).replace('{email}', `p${i - 1}@client.example`).replace('{sent}', 'Monday').replace('{to}', 'Milo').replace('{subject}', 'Avtale').replace('{previous}', inner)
    html = outlookReplyTemplate.replace('{name}', `Person ${i}`).replace('{title}', 'Innkjøp').replace('{company}', 'Kunde AS').replace('{domain}', 'client.example').replace('{logo}', 'https://www.client.example/logo.png').replace('{body}', `<p class="MsoNormal">Melding ${i}</p>`).replace('{quote}', quote)
  }
  return html
}

function gmailThread(depth: number): string {
  let html = ''
  for (let i = 0; i < depth; i++) {
    const quote = i === 0 ? '' : gmailQuoteTemplate.trim().replace('{sent}', 'Wed, 7 Oct 2026 at 10:02').replace('{name}', `Person ${i - 1}`).replace('{email}', `p${i - 1}@example.com`).replace('{previous}', html)
    html = gmailReplyTemplate.trim().replace('{name}', `Person ${i}`).replace('{title}', 'Designer').replace('{body}', `Melding ${i}`).replace('{quote}', quote)
  }
  return html
}

describe('quote detection per mail style in the mock threads', () => {
  it('folds an outlook chain with a logo signature behind the first divRplyFwdMsg', () => {
    const html = outlookThread(8)
    expect(html.match(/id="divRplyFwdMsg"/g)).toHaveLength(7)
    const parts = splitQuoted(html, true)
    expect(text(parts.visible)).toContain('Melding 7')
    expect(text(parts.visible)).not.toContain('Melding 6')
    expect(text(parts.quoted)).toContain('From: Person 6')
    expect(text(parts.signature)).toContain('Person 7')
    expect(parts.signature).toContain('https://www.client.example/logo.png')
  })

  it('folds a gmail chain nested three deep and its signature', () => {
    const html = gmailThread(4)
    expect(html.match(/<blockquote class="gmail_quote"/g)).toHaveLength(3)
    const parts = splitQuoted(html, true)
    expect(text(parts.visible).trim()).toBe('Melding 3')
    expect(text(parts.signature)).toContain('Person 3')
    expect(text(parts.quoted)).toContain('On Wed, 7 Oct 2026 at 10:02 Person 2')
    expect(parts.quoted.match(/<blockquote/g)).toHaveLength(3)
  })

  it('folds an apple mail reply at blockquote type cite', () => {
    const parts = splitQuoted(emlBody(appleEml), true)
    expect(text(parts.visible)).toContain('leverer alle seks som SVG')
    expect(text(parts.visible)).not.toContain('Ikonene ser kjempefine ut')
    expect(parts.quoted).toContain('type="cite"')
  })

  it('folds a hard wrapped plain text reply at the signature and the wrote header', () => {
    const parts = splitQuoted(emlBody(plainEml), false)
    expect(parts.visible.startsWith('Hei Milo,')).toBe(true)
    expect(parts.visible).toContain('Ta gjerne en runde med Anna')
    expect(parts.signature.split('\n')[0]).toBe('Hugo Brandt')
    expect(parts.quoted.split('\n')[0]).toMatch(/^On Fri, Oct 02, 2026 .+ wrote:$/)
    expect(parts.quoted.split('\n').filter((l) => l.startsWith('>')).every((l) => l.length <= 80)).toBe(true)
  })

  it('splits outlook original message and forwarded headers in plain text', () => {
    expect(splitQuoted('Se under.\n\n-----Original Message-----\nFrom: Kari\nHei', false).visible).toBe('Se under.')
    expect(splitQuoted('FYI\n---------- Forwarded message ---------\nFrom: GitHub', false).quoted.startsWith('----------')).toBe(true)
  })
})

describe('prepareMail', () => {
  it('resolves cid images to their attachment and leaves unknown cids empty', () => {
    const out = prepareMail('<p>Hei</p><img src="cid:logo@client.example"><img src="cid:missing@x">', { cids: { 'logo@client.example': 'data:image/png;base64,AAAA' } })
    const root = dom(out.html)
    const images = root.querySelectorAll('img')
    expect(images[0].getAttribute('src')).toBe('data:image/png;base64,AAAA')
    expect(images[1].hasAttribute('src')).toBe(false)
    expect(out.remote).toBe(0)
  })

  it('blocks remote images, backgrounds and stylesheets until allowed', () => {
    const html = '<link rel="stylesheet" href="https://x.test/a.css"><table background="https://x.test/bg.png"><tr><td style="background-image:url(https://x.test/td.png)"><img src="https://x.test/a.png" srcset="https://x.test/a2.png 2x"><img src="https://x.test/open.gif" width="1" height="1"></td></tr></table>'
    const blocked = prepareMail(html)
    expect(blocked.remote).toBe(5)
    expect(blocked.html).not.toMatch(/https:\/\/x\.test/)
    expect(dom(blocked.html).querySelectorAll('img[data-blocked]')).toHaveLength(2)
    const allowed = prepareMail(html, { remote: true })
    expect(allowed.html).toContain('https://x.test/a.png')
    expect(allowed.html).toContain('url(https://x.test/td.png)')
  })

  it('strips scripts, frames, forms, handlers and javascript links', () => {
    const out = prepareMail('<p onclick="x()">Hei</p><script>alert(1)</script><iframe src="https://x.test"></iframe><form action="https://x.test"><input></form><a href="javascript:alert(1)">a</a><meta http-equiv="refresh" content="0;url=https://x.test">')
    const root = dom(out.html)
    expect(root.querySelector('script, iframe, form, meta')).toBeNull()
    expect(root.querySelector('p')?.hasAttribute('onclick')).toBe(false)
    expect(root.querySelector('a')?.hasAttribute('href')).toBe(false)
  })

  it('marks designed mail and keeps plain replies dark mode friendly', () => {
    expect(prepareMail(emlBody(retailerEml)).designed).toBe(true)
    expect(prepareMail(emlBody(darkEml)).designed).toBe(true)
    expect(prepareMail(gmailThread(3)).designed).toBe(false)
    expect(prepareMail(emlBody(appleEml)).designed).toBe(false)
  })

  it('wraps the fold from the first quote or signature and only when text stays visible', () => {
    for (const html of [outlookThread(3), gmailThread(3), emlBody(appleEml)]) {
      const out = prepareMail(html)
      expect(out.foldable).toBe(true)
      const fold = dom(out.html).querySelector('.cn-fold')
      expect(fold?.textContent?.length).toBeGreaterThan(0)
    }
    const only = prepareMail('<blockquote>Bare sitat</blockquote>')
    expect(only.foldable).toBe(false)
    expect(dom(only.html).querySelector('.cn-fold')).toBeNull()
  })

  it('pulls blank spacers into the fold and counts the blocked images it hides', () => {
    const out = prepareMail('<p>Hei</p><p class="MsoNormal">&nbsp;</p><p><br></p><div id="Signature"><img src="https://x.test/logo.png"><p>Anna</p></div>')
    const body = dom(out.html)
    expect(body.firstElementChild?.textContent).toBe('Hei')
    expect(body.children).toHaveLength(2)
    expect(out.remote).toBe(1)
    expect(out.folded).toBe(1)
    expect(prepareMail('<p>Hei</p><img src="https://x.test/a.png"><div id="Signature">Anna</div>').folded).toBe(0)
  })

  it('treats coloured text as plain and only painted backgrounds as designed', () => {
    expect(prepareMail('<p>Hei</p><span style="color:#595959">Anna</span>').designed).toBe(false)
    expect(prepareMail('<div style="background-color:#123">Hei</div>').designed).toBe(true)
  })

  it('wraps top level tables for horizontal scroll and keeps nested tables in place', () => {
    const out = prepareMail(emlBody(retailerEml))
    const root = dom(out.html)
    const outer = root.querySelectorAll('.cn-scroll > table')
    expect(outer.length).toBeGreaterThan(0)
    expect(root.querySelectorAll('table .cn-scroll')).toHaveLength(0)
    expect(root.querySelector('.cn-body')?.getAttribute('style')).toContain('background-color:#f3efe8')
  })

  it('keeps the github diff table and drops the json-ld script', () => {
    const out = prepareMail(emlBody(githubEml).split('--gh-alt')[2] ?? '')
    expect(out.html).toContain('@@ -41,9 +41,14 @@')
    expect(out.html).not.toContain('application/ld+json')
  })
})

describe('mailDocument', () => {
  const plain = prepareMail('<p>Hei</p>')
  const designed = prepareMail('<table bgcolor="#ffffff"><tr><td>Hei</td></tr></table>')

  it('blocks remote loads in the CSP unless allowed and never allows scripts', () => {
    const blocked = mailDocument(plain, { remote: false, dark: false, ink: '#111', accent: '#00f', quote: false })
    expect(blocked).toContain("default-src 'none'")
    expect(blocked).toContain('img-src data: blob:;')
    expect(blocked).not.toContain('script-src')
    const allowed = mailDocument(plain, { remote: true, dark: false, ink: '#111', accent: '#00f', quote: false })
    expect(allowed).toContain('img-src data: blob: https: http:')
  })

  it('uses the app ink on plain mail and a fixed light ink on designed mail in dark mode', () => {
    expect(mailDocument(plain, { remote: false, dark: true, ink: 'rgb(244, 241, 236)', accent: '#9cf', quote: false })).toContain('color:rgb(244, 241, 236)')
    const inset = mailDocument(designed, { remote: false, dark: true, ink: 'rgb(244, 241, 236)', accent: '#9cf', quote: false })
    expect(inset).toContain('color:#1d1a17')
    expect(inset).not.toContain('max-width:68ch')
  })

  it('injects the base styles and the quote class', () => {
    const doc = mailDocument(plain, { remote: false, dark: false, ink: '#111', accent: '#00f', quote: true })
    expect(doc).toContain("'Schibsted Grotesk'")
    expect(doc).toContain('15px/1.6')
    expect(doc).toContain('max-width:68ch')
    expect(doc).toContain('img{max-width:100%')
    expect(doc).toContain('.cn-scroll{max-width:100%;overflow-x:auto}')
    expect(doc).toContain('<html class="cn-quote">')
  })
})

describe('lightSurface', () => {
  it('reads the first solid background and treats transparent mail as light', async () => {
    const { lightSurface, colorChannels } = await import('./content')
    expect(colorChannels('rgba(10, 20, 30, 0.5)')).toEqual({ r: 10, g: 20, b: 30, a: 0.5 })
    expect(lightSurface(['rgba(0, 0, 0, 0)', 'rgb(255, 255, 255)'])).toBe(true)
    expect(lightSurface(['rgb(250, 248, 244)'])).toBe(true)
    expect(lightSurface(['rgb(43, 36, 28)', 'rgb(255, 255, 255)'])).toBe(false)
    expect(lightSurface(['rgba(0, 0, 0, 0)'])).toBe(true)
    expect(lightSurface([])).toBe(true)
  })
})
