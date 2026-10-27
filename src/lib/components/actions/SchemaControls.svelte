<script lang="ts">
  import Icon from '@iconify/svelte'
  import SchemaControls from './SchemaControls.svelte'
  import { Select, Switch } from '$lib/components/ui'
  import { typeOf, type JsonSchema } from '$lib/actions'

  interface Props {
    schema: JsonSchema
    value: Record<string, unknown>
    size?: 'sm' | 'md'
    testid?: string
    onchange: (next: Record<string, unknown>) => void
  }

  let { schema, value, size = 'md', testid, onchange }: Props = $props()

  const fields = $derived(Object.entries(schema.properties ?? {}))
  const heights = { sm: 'h-6 text-[11.5px]', md: 'h-8 text-[12.5px]' }
  const labels = { sm: 'text-[10.5px]', md: 'text-[12px]' }

  function put(name: string, next: unknown) {
    onchange({ ...value, [name]: next })
  }

  function text(raw: unknown): string {
    if (raw === null || raw === undefined) return ''
    return Array.isArray(raw) ? raw.join(', ') : String(raw)
  }

  function number(raw: string): number | null {
    return raw === '' ? null : Number(raw)
  }

  function list(raw: string): string[] {
    return raw.split(',').map((s) => s.trim()).filter(Boolean)
  }

  function toggle(name: string, current: unknown[], item: unknown) {
    put(name, current.includes(item) ? current.filter((v) => v !== item) : [...current, item].sort())
  }

  function record(raw: unknown): Record<string, unknown> {
    return raw !== null && typeof raw === 'object' && !Array.isArray(raw) ? (raw as Record<string, unknown>) : {}
  }
</script>

<div class="flex min-w-0 flex-col gap-1" data-testid={testid}>
  {#each fields as [name, field] (name)}
    {@const kind = typeOf(field)}
    {@const current = value[name]}
    {@const items = field.items}
    {#if kind === 'object'}
      <div class="flex flex-col gap-1">
        <span class="font-mono uppercase tracking-wide text-ink-3 {labels[size]}">{field.title ?? name}</span>
        <div class="border-l border-line pl-2">
          <SchemaControls schema={field} value={record(current)} {size} onchange={(next) => put(name, next)} />
        </div>
      </div>
    {:else if kind === 'array' && items?.enum}
      <div class="grid grid-cols-[minmax(0,0.6fr)_minmax(0,1fr)] items-center gap-2" data-field={name}>
        <span class="overflow-hidden text-ellipsis whitespace-nowrap text-ink-2 {labels[size]}" title={field.description ?? name}>{field.title ?? name}</span>
        <div class="flex gap-0.5">
          {#each items.enum as item, i (String(item))}
            {@const on = Array.isArray(current) && current.includes(item)}
            <button type="button" class="flex-1 rounded-[0.375rem] {heights[size]} {on ? 'bg-accent-soft text-ink' : 'text-ink-3 hover:bg-paper-3'}" aria-pressed={on} onclick={() => toggle(name, Array.isArray(current) ? current : [], item)}>{items.labels?.[i] ?? String(item)}</button>
          {/each}
        </div>
      </div>
    {:else}
      <label class="grid grid-cols-[minmax(0,0.6fr)_minmax(0,1fr)] items-center gap-2" data-field={name}>
        <span class="flex min-w-0 items-center gap-1 overflow-hidden text-ellipsis whitespace-nowrap text-ink-2 {labels[size]}" title={field.description ?? name}>{field.title ?? name}{#if field.format}<span class="font-mono text-ink-3">{field.format}</span>{/if}</span>
        {#if kind === 'boolean'}
          <Switch label={field.title ?? name} checked={current === true} onchange={(next) => put(name, next)} />
        {:else if field.enum}
          <Select {size} value={text(current)} options={field.enum.map((item, i) => ({ value: String(item), label: field.labels?.[i] ?? String(item) }))} onchange={(v) => put(name, field.enum?.find((item) => String(item) === v) ?? v)} />
        {:else if kind === 'integer' || kind === 'number'}
          <input type="number" min={field.minimum} max={field.maximum} step={kind === 'integer' ? 1 : 'any'} class="min-w-0 border-b border-line bg-transparent font-mono text-ink outline-none focus:border-ink-3 {heights[size]}" value={text(current)} oninput={(e) => put(name, number(e.currentTarget.value))} />
        {:else if kind === 'array'}
          <input type="text" class="min-w-0 border-b border-line bg-transparent font-mono text-ink outline-none focus:border-ink-3 {heights[size]}" value={text(current)} oninput={(e) => put(name, list(e.currentTarget.value))} />
        {:else}
          <input type={field.format === 'time' ? 'time' : field.format === 'email' ? 'email' : 'text'} class="min-w-0 border-b border-line bg-transparent text-ink outline-none focus:border-ink-3 {heights[size]} {field.format ? 'font-mono' : ''}" value={text(current)} oninput={(e) => put(name, e.currentTarget.value === '' ? null : e.currentTarget.value)} />
        {/if}
      </label>
    {/if}
  {/each}
  {#if !fields.length}
    <span class="flex items-center gap-1 text-ink-3 {labels[size]}"><Icon icon="lucide:circle-dashed" />{schema.title ?? ''}</span>
  {/if}
</div>
