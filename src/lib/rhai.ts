import { StreamLanguage, type StringStream, LanguageSupport } from '@codemirror/language'
import { snippetCompletion, type Completion, type CompletionContext, type CompletionResult } from '@codemirror/autocomplete'
import { tags } from '@lezer/highlight'
import type { ActionFunction, ActionPort } from '$lib/tauri/types'

export const KEYWORDS = ['let', 'const', 'if', 'else', 'switch', 'while', 'loop', 'for', 'in', 'do', 'until', 'break', 'continue', 'return', 'throw', 'try', 'catch', 'fn', 'private', 'true', 'false', 'this', 'is_def_var', 'type_of']
const KEYWORD_SET = new Set(KEYWORDS)

interface RhaiState {
  raw: number
  block: boolean
  backtick: boolean
}

function readRaw(stream: StringStream, state: RhaiState): string {
  while (!stream.eol()) {
    const ch = stream.next()
    if (ch === '"') {
      let hashes = 0
      while (hashes < state.raw && stream.peek() === '#') {
        stream.next()
        hashes += 1
      }
      if (hashes === state.raw) {
        state.raw = 0
        return 'string'
      }
    }
  }
  return 'string'
}

export const rhaiLanguage = StreamLanguage.define<RhaiState>({
  name: 'rhai',
  startState: () => ({ raw: 0, block: false, backtick: false }),
  token(stream, state) {
    if (state.raw) return readRaw(stream, state)
    if (state.block) {
      if (stream.skipTo('*/')) {
        stream.match('*/')
        state.block = false
      } else stream.skipToEnd()
      return 'comment'
    }
    if (state.backtick) {
      while (!stream.eol()) {
        if (stream.next() === '`') {
          state.backtick = false
          break
        }
      }
      return 'string'
    }
    if (stream.eatSpace()) return null
    if (stream.match('//')) {
      stream.skipToEnd()
      return 'comment'
    }
    if (stream.match('/*')) {
      state.block = true
      return 'comment'
    }
    const raw = stream.match(/^#+"/) as RegExpMatchArray | null
    if (raw) {
      state.raw = raw[0].length - 1
      return readRaw(stream, state)
    }
    if (stream.match('#{')) return 'bracket'
    if (stream.match(/^"(?:[^"\\]|\\.)*"?/)) return 'string'
    if (stream.match(/^'(?:[^'\\]|\\.)*'?/)) return 'string'
    if (stream.peek() === '`') {
      stream.next()
      state.backtick = true
      while (!stream.eol()) {
        if (stream.next() === '`') {
          state.backtick = false
          break
        }
      }
      return 'string'
    }
    if (stream.match(/^(?:0x[0-9a-fA-F_]+|0b[01_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:e[+-]?\d+)?)/)) return 'number'
    const word = stream.match(/^[A-Za-z_][A-Za-z0-9_]*/) as RegExpMatchArray | null
    if (word) {
      if (KEYWORD_SET.has(word[0])) return word[0] === 'true' || word[0] === 'false' ? 'atom' : 'keyword'
      if (stream.match(/^\s*\(/, false)) return 'function'
      return 'variableName'
    }
    if (stream.match(/^(?:==|!=|<=|>=|&&|\|\||\?\?|\.\.=?|[-+*/%=<>!&|^?:.])/)) return 'operator'
    stream.next()
    return null
  },
  languageData: { commentTokens: { line: '//', block: { open: '/*', close: '*/' } }, closeBrackets: { brackets: ['(', '[', '{', '"', '`'] } },
  tokenTable: { function: tags.function(tags.variableName) }
})

export const SNIPPETS: Completion[] = [
  snippetCompletion('if ${condition} {\n\t${}\n}', { label: 'if', detail: 'block', type: 'keyword' }),
  snippetCompletion('if ${condition} {\n\t${}\n} else {\n\t${}\n}', { label: 'if', detail: 'else block', type: 'keyword' }),
  snippetCompletion('for ${item} in ${list} {\n\t${}\n}', { label: 'for', detail: 'loop', type: 'keyword' }),
  snippetCompletion('let ${name} = ${value};', { label: 'let', detail: 'variable', type: 'keyword' }),
  snippetCompletion('#{ ${name}: ${value} }', { label: '#{', detail: 'output map', type: 'keyword' }),
  snippetCompletion('node("${id}", "${type}", #{ "label": "${label}", "x": 0.0, "y": 0.0, "config": #{} }, #"\n${}\n"#);', { label: 'node', detail: 'action code: a node', type: 'keyword' }),
  snippetCompletion('connect("${source}", "${output}", "${target}", "${input}");', { label: 'connect', detail: 'action code: a wire', type: 'keyword' })
]

export function rhaiCompletions(functions: () => ActionFunction[], inputs: () => ActionPort[]) {
  return (context: CompletionContext): CompletionResult | null => {
    const word = context.matchBefore(/[A-Za-z_][A-Za-z0-9_]*/)
    if (!word && !context.explicit) return null
    const options: Completion[] = [
      ...functions().map((f) => ({ label: f.name, type: 'function', detail: f.signature.slice(f.name.length), info: f.effect ? `${f.description} Changes things; dry runs only describe it.` : f.description, boost: 2 })),
      ...inputs().map((p) => ({ label: p.name, type: 'variable', detail: p.type, info: p.description || undefined, boost: 3 })),
      ...KEYWORDS.map((k) => ({ label: k, type: 'keyword' })),
      ...SNIPPETS
    ]
    return { from: word ? word.from : context.pos, options, validFor: /^[A-Za-z_][A-Za-z0-9_]*$/ }
  }
}

export function rhai(): LanguageSupport {
  return new LanguageSupport(rhaiLanguage)
}

export function lineOffset(text: string, line: number | null, column: number | null): number {
  if (!line) return 0
  const lines = text.split('\n')
  let offset = 0
  for (let i = 0; i < Math.min(line - 1, lines.length); i += 1) offset += lines[i].length + 1
  return Math.min(offset + Math.max((column ?? 1) - 1, 0), text.length)
}
