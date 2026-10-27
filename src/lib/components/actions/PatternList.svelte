<script lang="ts">
  import { IconButton } from '$lib/components/ui'
  import { validRegex } from '$lib/tauri/groups'
  import { t } from '$lib/i18n.svelte'

  interface Pattern {
    name: string
    pattern: string
    weight: number
    fields?: string[]
  }

  interface Props {
    patterns: unknown[]
    onchange: (patterns: Pattern[]) => void
  }

  let { patterns, onchange }: Props = $props()

  const list = $derived(patterns.filter((p): p is Pattern => typeof p === 'object' && p !== null && 'pattern' in p))

  function set(index: number, patch: Partial<Pattern>) {
    onchange(list.map((p, i) => (i === index ? { ...p, ...patch } : p)))
  }
</script>

<div class="flex flex-col gap-1.5">
  <div class="flex h-7 items-center justify-between">
    <span class="text-[12px] font-medium uppercase tracking-wide text-ink-3">{t('actions.patterns.title')} <span class="font-mono normal-case">{list.length}</span></span>
    <IconButton size="sm" icon="lucide:plus" label={t('actions.patterns.add')} onclick={() => onchange([...list, { name: t('actions.patterns.newName'), pattern: '', weight: 0.2 }])} />
  </div>
  <p class="text-[12px] leading-5 text-ink-3">{t('actions.patterns.hint')}</p>
  <ul class="flex flex-col">
    {#each list as pattern, i (i)}
      {@const problem = validRegex(pattern.pattern)}
      <li class="grid grid-cols-[minmax(0,1fr)_4.5em_auto] items-center gap-x-2 border-b border-line py-1">
        <input class="h-7 min-w-0 bg-transparent text-[12.5px] text-ink outline-none placeholder:text-ink-3" value={pattern.name} aria-label={t('actions.patterns.name')} oninput={(e) => set(i, { name: e.currentTarget.value })} />
        <input type="number" step="0.05" min="-1" max="1" class="h-7 min-w-0 bg-transparent text-right font-mono text-[12px] {pattern.weight < 0 ? 'text-error-600' : 'text-ink'} outline-none" value={pattern.weight} aria-label={t('actions.patterns.weight')} oninput={(e) => set(i, { weight: Number(e.currentTarget.value) })} />
        <IconButton size="sm" icon="lucide:x" label={t('actions.patterns.remove', { name: pattern.name })} onclick={() => onchange(list.filter((_, j) => j !== i))} />
        <input class="col-span-3 h-7 min-w-0 bg-transparent font-mono text-[11.5px] text-ink-2 outline-none placeholder:text-ink-3" value={pattern.pattern} placeholder={t('actions.patterns.placeholder')} aria-label={t('actions.patterns.pattern')} oninput={(e) => set(i, { pattern: e.currentTarget.value })} />
        {#if problem}<p class="col-span-3 text-[11.5px] text-error-600">{problem}</p>{/if}
      </li>
    {/each}
  </ul>
</div>
