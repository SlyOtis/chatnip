<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte'
  import { EditorView, basicSetup } from 'codemirror'
  import { EditorState, Compartment, type Extension } from '@codemirror/state'
  import { keymap } from '@codemirror/view'
  import { indentWithTab } from '@codemirror/commands'
  import { autocompletion } from '@codemirror/autocomplete'
  import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
  import { lintGutter, setDiagnostics, type Diagnostic as CmDiagnostic } from '@codemirror/lint'
  import { unifiedMergeView } from '@codemirror/merge'
  import { html } from '@codemirror/lang-html'
  import { json } from '@codemirror/lang-json'
  import { javascript } from '@codemirror/lang-javascript'
  import { oneDark, color } from '@codemirror/theme-one-dark'
  import { tags as t } from '@lezer/highlight'
  import { rhai, rhaiCompletions, lineOffset } from '$lib/rhai'
  import type { ActionDiagnostic, ActionFunction, ActionPort } from '$lib/tauri/types'

  type Language = 'rhai' | 'svelte' | 'html' | 'json' | 'javascript' | 'text'
  type Scheme = 'day' | 'night'

  interface Props {
    value: string
    readonly?: boolean
    functions?: ActionFunction[]
    inputs?: ActionPort[]
    diagnostics?: ActionDiagnostic[]
    original?: string | null
    language?: Language
    label: string
    onchange?: (value: string) => void
  }

  let { value = $bindable(), readonly = false, functions = [], inputs = [], diagnostics = [], original = null, language = 'rhai', label, onchange }: Props = $props()

  let host = $state<HTMLDivElement | null>(null)
  let view: EditorView | null = null
  let observer: MutationObserver | null = null
  const editable = new Compartment()
  const merge = new Compartment()
  const syntax = new Compartment()
  const palette = new Compartment()

  const LIGHT = { background: '#fafafa', foreground: '#383a42', gutter: '#9d9d9f', keyword: '#a626a4', literal: '#986801', string: '#50a14f', escape: '#0184bc', comment: '#a0a1a7', function: '#4078f2', name: '#c18401', tag: '#e45649', cursor: '#526fff', selection: '#e5e5e6', tooltip: '#ffffff' }
  const SURFACE: Record<Scheme, string> = { day: LIGHT.background, night: color.background }

  const lightHighlight = HighlightStyle.define([
    { tag: [t.keyword, t.controlKeyword, t.moduleKeyword, t.definitionKeyword, t.operatorKeyword], color: LIGHT.keyword, fontWeight: '600' },
    { tag: [t.atom, t.bool, t.null, t.number], color: LIGHT.literal },
    { tag: [t.string, t.special(t.string), t.regexp], color: LIGHT.string },
    { tag: [t.escape, t.character], color: LIGHT.escape },
    { tag: [t.comment, t.lineComment, t.blockComment], color: LIGHT.comment, fontStyle: 'italic' },
    { tag: [t.function(t.variableName), t.function(t.propertyName)], color: LIGHT.function },
    { tag: [t.variableName, t.name, t.propertyName, t.attributeName], color: LIGHT.name },
    { tag: [t.typeName, t.className, t.namespace], color: LIGHT.name, fontWeight: '600' },
    { tag: [t.tagName, t.angleBracket], color: LIGHT.tag },
    { tag: [t.operator, t.derefOperator, t.compareOperator, t.arithmeticOperator, t.logicOperator, t.bitwiseOperator, t.updateOperator], color: LIGHT.escape },
    { tag: [t.punctuation, t.separator, t.bracket, t.brace, t.paren, t.squareBracket], color: LIGHT.foreground },
    { tag: t.meta, color: LIGHT.keyword },
    { tag: [t.url, t.link], color: LIGHT.function, textDecoration: 'underline' },
    { tag: t.invalid, color: LIGHT.tag, textDecoration: 'underline wavy' },
    { tag: [t.heading, t.strong], fontWeight: 'bold' },
    { tag: t.emphasis, fontStyle: 'italic' },
    { tag: t.strikethrough, textDecoration: 'line-through' }
  ])

  const lightTheme = EditorView.theme(
    {
      '&': { backgroundColor: LIGHT.background, color: LIGHT.foreground },
      '.cm-content': { caretColor: LIGHT.cursor },
      '.cm-cursor, .cm-dropCursor': { borderLeftColor: LIGHT.cursor },
      '.cm-gutters': { backgroundColor: LIGHT.background, color: LIGHT.gutter, border: 'none' },
      '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'rgb(0 0 0 / 0.04)' },
      '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': { backgroundColor: LIGHT.selection },
      '.cm-tooltip': { backgroundColor: LIGHT.tooltip, color: LIGHT.foreground, border: `1px solid ${LIGHT.selection}` },
      '.cm-tooltip-autocomplete ul li[aria-selected]': { backgroundColor: LIGHT.selection, color: LIGHT.foreground }
    },
    { dark: false }
  )

  const schemes: Record<Scheme, Extension> = { day: [lightTheme, syntaxHighlighting(lightHighlight)], night: oneDark }

  const theme = EditorView.theme({
    '&': { height: '100%', maxHeight: 'inherit' },
    '.cm-scroller': { fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace', fontSize: '0.75rem', lineHeight: '1.65', maxHeight: 'inherit', overflow: 'auto' },
    '.cm-content': { padding: '0.625rem 0' },
    '.cm-cursor': { borderLeftWidth: '2px' },
    '.cm-gutters': { paddingRight: '0.5rem' },
    '&.cm-focused': { outline: 'none' },
    '.cm-tooltip': { borderRadius: '0.5rem' },
    '.cm-completionDetail': { fontStyle: 'normal', marginLeft: '0.5rem', opacity: '0.7' },
    '.cm-changedLine': { backgroundColor: 'color-mix(in srgb, var(--accent) 10%, transparent)' },
    '.cm-deletedChunk': { backgroundColor: 'color-mix(in srgb, var(--color-error-500, #d33) 10%, transparent)' }
  })

  function current(): Scheme {
    return typeof document !== 'undefined' && document.documentElement.dataset.theme === 'night' ? 'night' : 'day'
  }

  let scheme = $state<Scheme>(current())

  function languageExtension(lang: Language): Extension {
    if (lang === 'rhai') return [rhai(), autocompletion({ override: [rhaiCompletions(() => functions, () => inputs)] })]
    if (lang === 'json') return json()
    if (lang === 'javascript') return javascript()
    if (lang === 'svelte' || lang === 'html') return html({ selfClosingTags: true })
    return []
  }

  function mergeExtension(text: string | null) {
    return text === null ? [] : unifiedMergeView({ original: text, mergeControls: false, highlightChanges: true, gutter: true })
  }

  function push(list: ActionDiagnostic[]) {
    if (!view) return
    const doc = view.state.doc.toString()
    const marks: CmDiagnostic[] = list.map((d) => {
      const from = lineOffset(doc, d.line, d.column)
      return { from, to: Math.min(doc.length, Math.max(from + 1, view!.state.doc.lineAt(from).to)), severity: d.severity === 'warning' ? 'warning' : 'error', message: d.message }
    })
    view.dispatch(setDiagnostics(view.state, marks))
  }

  export function insert(text: string) {
    if (!view || readonly) return
    const at = view.state.selection.main.head
    view.dispatch({ changes: { from: at, insert: text }, selection: { anchor: at + text.length } })
    view.focus()
  }

  onMount(() => {
    if (!host) return
    observer = new MutationObserver(() => (scheme = current()))
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value ?? '',
        extensions: [
          basicSetup,
          keymap.of([indentWithTab]),
          syntax.of(languageExtension(language)),
          lintGutter(),
          theme,
          palette.of(schemes[scheme]),
          editable.of([EditorState.readOnly.of(readonly), EditorView.editable.of(!readonly)]),
          merge.of(mergeExtension(original)),
          EditorView.contentAttributes.of({ 'aria-label': label }),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged) return
            const next = update.state.doc.toString()
            if (next === value) return
            value = next
            onchange?.(next)
          })
        ]
      })
    })
    push(diagnostics)
  })

  onDestroy(() => {
    observer?.disconnect()
    view?.destroy()
  })

  $effect(() => {
    const next = value ?? ''
    untrack(() => {
      if (view && next !== view.state.doc.toString()) view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } })
    })
  })

  $effect(() => {
    const locked = readonly
    untrack(() => view?.dispatch({ effects: editable.reconfigure([EditorState.readOnly.of(locked), EditorView.editable.of(!locked)]) }))
  })

  $effect(() => {
    const lang = language
    untrack(() => view?.dispatch({ effects: syntax.reconfigure(languageExtension(lang)) }))
  })

  $effect(() => {
    const next = scheme
    untrack(() => view?.dispatch({ effects: palette.reconfigure(schemes[next]) }))
  })

  $effect(() => {
    const text = original
    untrack(() => view?.dispatch({ effects: merge.reconfigure(mergeExtension(text)) }))
  })

  $effect(() => {
    const list = diagnostics
    untrack(() => push(list))
  })
</script>

<div bind:this={host} class="h-full max-h-[inherit] min-h-0 w-full overflow-hidden rounded-[0.5rem]" style:background-color={SURFACE[scheme]}></div>
