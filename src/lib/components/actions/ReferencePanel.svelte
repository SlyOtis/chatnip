<script lang="ts">
  import type { ActionFunction, ActionPort } from '$lib/tauri/types'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    functions: ActionFunction[]
    inputs: ActionPort[]
    oninsert: (text: string) => void
  }

  let { functions, inputs, oninsert }: Props = $props()
  let query = $state('')

  const shown = $derived(functions.filter((f) => `${f.name} ${f.description}`.toLowerCase().includes(query.trim().toLowerCase())))

  function call(f: ActionFunction): string {
    return `${f.name}(${f.params.slice(0, f.required).map(([name]) => name).join(', ')})`
  }
</script>

<div class="flex min-h-0 flex-col gap-2">
  <input
    class="h-8 w-full border-b border-line bg-transparent text-[12.5px] text-ink outline-none placeholder:text-ink-3 focus:border-ink-3"
    placeholder={t('actions.reference.find')}
    aria-label={t('actions.reference.find')}
    bind:value={query}
  />
  {#if inputs.length}
    <div class="flex flex-wrap gap-1">
      {#each inputs as port (port.name)}
        <button type="button" class="rounded-[0.375rem] bg-paper-3 px-1.5 py-0.5 font-mono text-[11.5px] text-ink-2 hover:text-ink" title="{port.type}{port.description ? ` · ${port.description}` : ''}" onclick={() => oninsert(port.name)}>{port.name}</button>
      {/each}
    </div>
  {/if}
  <ul class="no-scrollbar flex min-h-0 flex-col overflow-y-auto">
    {#each shown as f (f.name)}
      <li>
        <button type="button" class="flex w-full flex-col gap-0.5 rounded-[0.375rem] px-1.5 py-1.5 text-left hover:bg-paper-3" onclick={() => oninsert(call(f))}>
          <span class="font-mono text-[11.5px] text-ink">{f.signature}{#if f.effect}<span class="ml-1 text-signal" title={t('actions.reference.effect')}>●</span>{/if}</span>
          <span class="text-[11.5px] leading-4 text-ink-3">{f.description}</span>
        </button>
      </li>
    {/each}
  </ul>
</div>
