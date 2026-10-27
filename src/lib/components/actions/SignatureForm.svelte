<script lang="ts">
  import { IconButton, Select } from '$lib/components/ui'
  import type { ActionPort, PortType } from '$lib/tauri/types'
  import { uniqueId, type OutputSource } from '$lib/actions'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    title: string
    ports: ActionPort[]
    types: PortType[]
    readonly?: boolean
    fixed?: boolean
    origins?: (OutputSource | null)[]
    onorigin?: (node: string) => void
    onchange: (ports: ActionPort[]) => void
  }

  let { title, ports, types, readonly = false, fixed = false, origins = [], onorigin, onchange }: Props = $props()

  const options = $derived(types.map((type) => ({ value: type, label: type })))
  const shaped = $derived(readonly || fixed)

  function update(index: number, patch: Partial<ActionPort>) {
    onchange(ports.map((p, i) => (i === index ? { ...p, ...patch } : p)))
  }

  function add() {
    onchange([...ports, { name: uniqueId('value', ports.map((p) => p.name)), type: 'text', description: '' }])
  }
</script>

<div class="flex flex-col gap-1">
  <div class="flex h-7 items-center justify-between">
    <span class="text-[12px] font-medium uppercase tracking-wide text-ink-3">{title}</span>
    {#if !shaped}
      <IconButton size="sm" icon="lucide:plus" label={t('actions.signature.add', { what: title.toLowerCase() })} onclick={add} />
    {/if}
  </div>
  {#if !ports.length}
    <p class="text-[12.5px] text-ink-3">{t('actions.signature.none')}</p>
  {/if}
  {#each ports as port, index (index)}
    {@const origin = origins[index] ?? null}
    <div class="grid grid-cols-[minmax(0,1fr)_minmax(0,0.8fr)_auto] items-center gap-2">
      <input
        class="h-8 min-w-0 border-b border-line bg-transparent font-mono text-[12.5px] text-ink outline-none placeholder:text-ink-3 focus:border-ink-3"
        value={port.name}
        disabled={readonly}
        aria-label={t('common.name')}
        oninput={(e) => update(index, { name: e.currentTarget.value.replace(/[^A-Za-z0-9_]/g, '_') })}
      />
      {#if shaped}
        <span class="font-mono text-[12px] text-ink-3">{port.type}</span>
      {:else}
        <Select size="sm" value={port.type} {options} onchange={(type) => update(index, { type: type as PortType })} />
      {/if}
      {#if !shaped}
        <IconButton size="sm" icon="lucide:x" label={t('actions.list.remove', { name: port.name })} onclick={() => onchange(ports.filter((_, i) => i !== index))} />
      {:else if origin}
        <button type="button" class="max-w-[8rem] overflow-hidden text-ellipsis whitespace-nowrap text-right text-[12px] text-ink-3 hover:text-ink hover:underline" title={t('actions.signature.select', { node: origin.label })} disabled={!onorigin} onclick={() => onorigin?.(origin.node)}>{t('actions.signature.from', { node: origin.label })}</button>
      {:else}
        <span></span>
      {/if}
    </div>
    <input
      class="h-7 w-full border-b border-transparent bg-transparent text-[12px] text-ink-2 outline-none placeholder:text-ink-3 focus:border-line"
      value={port.description ?? ''}
      disabled={readonly}
      placeholder={t('actions.signature.descriptionPlaceholder')}
      aria-label={t('actions.signature.description', { name: port.name })}
      oninput={(e) => update(index, { description: e.currentTarget.value })}
    />
  {/each}
</div>
