<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri'
  import type { ContentPrompt, IntelligenceConnection, Prompt, PromptOverride } from '$lib/tauri/types'
  import { insertVariable, missingVariables, originName, parseSchema } from '$lib/intelligence'
  import { promptOverride } from '$lib/actions'
  import { t } from '$lib/i18n.svelte'
  import { Button, Select, Sheet } from '$lib/components/ui'

  interface Props {
    prompt: Prompt
    base: Prompt
    node: string
    contentId?: string | null
    contentPrompts?: ContentPrompt[]
    note?: string | null
    onsave: (override: PromptOverride | null) => void
    onclose: () => void
  }

  let { prompt, base, node, contentId = null, contentPrompts = [], note = null, onsave, onclose }: Props = $props()

  let source = $state(contentId && contentPrompts.some((entry) => entry.id === contentId) ? contentId : '')
  let template = $state(contentId ? base.template : prompt.template)
  let system = $state(prompt.system)
  let schema = $state(prompt.output_schema ?? '')
  let connectionId = $state(prompt.connection_id ?? '')
  let model = $state(prompt.model ?? '')
  let connections = $state<IntelligenceConnection[]>([])
  let editor = $state<HTMLTextAreaElement | null>(null)
  let failure = $state<string | null>(null)

  const chosen = $derived(contentPrompts.find((entry) => entry.id === source) ?? null)
  const shown = $derived(chosen ? chosen.body : template)
  const missing = $derived(missingVariables(shown, base.variables.map((variable) => variable.name.split('.')[0])))
  const origin = $derived(originName({ connection_id: connectionId || null }, connections))
  const area = 'w-full resize-y rounded-[0.25rem] bg-paper-2 px-3 py-2 font-mono text-[12.5px] leading-5 text-ink outline-none placeholder:text-ink-3'

  onMount(() => {
    api.getIntelligenceConnections().then((list) => (connections = list)).catch(() => {})
  })

  function insert(variable: string) {
    const cursor = editor?.selectionStart ?? template.length
    const next = insertVariable(template, variable, cursor)
    template = next.template
    requestAnimationFrame(() => {
      editor?.focus()
      editor?.setSelectionRange(next.cursor, next.cursor)
    })
  }

  function save() {
    failure = null
    try {
      onsave(promptOverride(base, { system, template, content_id: source || null, output_schema: parseSchema(schema), connection_id: connectionId || null, model: model || null }))
    } catch (error) {
      failure = String(error)
    }
  }

  function reset() {
    system = base.system
    template = base.template
    source = ''
    schema = base.output_schema ?? ''
    connectionId = ''
    model = ''
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) save()
  }
</script>

<Sheet label={t('actions.prompt.label', { name: base.name })} width="52rem" {onclose}>
  <div class="flex flex-col gap-5 px-5 py-4" onkeydown={keydown} role="presentation">
    <div class="flex items-start justify-between gap-4">
      <div class="min-w-0 flex-1">
        <p class="text-[15px] font-semibold tracking-tight text-ink">{base.name}</p>
        <p class="mt-0.5 text-[13px] text-ink-3">{base.description} {note ?? t('actions.prompt.scope', { node })}</p>
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <span class="font-mono text-[11px] text-ink-3">{base.key}</span>
      </div>
    </div>

    <div class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-3">
      <label class="flex flex-col gap-1 text-[12px] text-ink-3">{t('actions.prompt.connection')}
        <Select size="sm" value={connectionId} options={[{ value: '', label: t('actions.prompt.defaultConnection', { name: originName({ connection_id: null }, connections) }) }, ...connections.map((c) => ({ value: c.id, label: c.name }))]} onchange={(id) => (connectionId = id)} />
      </label>
      <label class="flex flex-col gap-1 text-[12px] text-ink-3">{t('actions.prompt.model')}
        <input class="h-7 border-b border-line bg-transparent font-mono text-[12.5px] text-ink outline-none placeholder:text-ink-3 focus:border-ink-3" placeholder={t('actions.prompt.modelPlaceholder')} bind:value={model} />
      </label>
    </div>

    <div class="grid grid-cols-1 gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(0,18rem)]">
      <div class="flex flex-col gap-2">
        <div class="flex items-baseline gap-2">
          <span class="text-[13px] font-medium text-ink-2">{t('actions.prompt.system')}</span>
          <span class="text-[12px] text-ink-3">{t('actions.prompt.systemHint')}</span>
        </div>
        <textarea bind:value={system} class="{area} min-h-[6rem]" spellcheck="false"></textarea>
        <div class="flex items-baseline gap-2">
          <span class="text-[13px] font-medium text-ink-2">{t('actions.prompt.prompt')}</span>
          <span class="text-[12px] text-ink-3">{t('actions.prompt.promptHint')}</span>
        </div>
        <label class="flex items-center gap-2 text-[12px] text-ink-3">{t('actions.prompt.source')}
          <Select size="sm" class="min-w-0 flex-1" bind:value={source} options={[{ value: '', label: t('actions.prompt.ownText') }, ...contentPrompts.map((entry) => ({ value: entry.id, label: t('actions.prompt.fromContent', { name: entry.name }) }))]} />
        </label>
        {#if chosen}
          <textarea value={chosen.body} class="{area} min-h-[18rem] text-ink-2" spellcheck="false" readonly aria-label={t('actions.prompt.prompt')}></textarea>
          <p class="text-[12px] text-ink-3">{t('actions.prompt.contentHint')} <a class="text-accent hover:underline" href="/main/content">{t('content.title')}</a></p>
        {:else}
          <textarea bind:this={editor} bind:value={template} class="{area} min-h-[18rem]" spellcheck="false"></textarea>
          <div class="flex flex-wrap gap-0.5">
            {#each base.variables as variable (variable.name)}
              <button type="button" class="h-6 rounded-[0.5rem] px-1.5 font-mono text-[11px] text-ink-2 hover:bg-paper-3 hover:text-ink" onclick={() => insert(variable.name)}>{'{{ .' + variable.name + ' }}'}</button>
            {/each}
          </div>
        {/if}
        {#if missing.length > 0}
          <p class="text-[12px] text-warn-600">{t('actions.prompt.missing', { names: missing.map((v) => `.${v}`).join(', ') })}</p>
        {/if}
      </div>
      <div class="flex flex-col gap-2">
        <span class="text-[13px] font-medium text-ink-2">{t('actions.prompt.schema')}</span>
        <textarea bind:value={schema} class="{area} min-h-[18rem] text-[12px]" placeholder={'{ "type": "object", "properties": { } }'} spellcheck="false"></textarea>
        <span class="text-[12px] text-ink-3">{t('actions.prompt.schemaHint')}</span>
      </div>
    </div>

    {#if failure}
      <p class="text-[13px] text-error-500">{failure}</p>
    {/if}
  </div>
  {#snippet footer()}
    <Button icon="lucide:rotate-ccw" onclick={reset}>{t('actions.prompt.useDefault')}</Button>
    <span class="flex-1 font-mono text-[11px] text-ink-3">{origin}{model ? ` · ${model}` : ''}</span>
    <Button onclick={onclose}>{t('common.cancel')}</Button>
    <Button variant="primary" keys="⌘↵" onclick={save}>{t('common.apply')}</Button>
  {/snippet}
</Sheet>
