<script lang="ts">
  import Icon from '@iconify/svelte'
  import type { Action } from '$lib/tauri/types'
  import { RECEIPTS_KEY } from '$lib/tauri/chats'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    actions: Action[]
    chosen: string[]
    copyReceipts: boolean
    onchange: (chosen: string[]) => void
    oncopy: (on: boolean) => void
  }

  let { actions, chosen, copyReceipts, onchange, oncopy }: Props = $props()

  const attachable = $derived(actions.filter((a) => a.usage.chat))
  const receipts = $derived(actions.find((a) => a.key === RECEIPTS_KEY) ?? null)

  function toggle(id: string) {
    onchange(chosen.includes(id) ? chosen.filter((c) => c !== id) : [...chosen, id])
  }
</script>

<ul class="flex flex-col">
  {#if receipts}
    <li>
      <button type="button" class="grid h-9 w-full grid-cols-[1.5rem_minmax(0,1fr)_1rem] items-center gap-1.5 rounded-[0.375rem] px-1 text-left text-[13px] hover:bg-paper-3 {copyReceipts ? 'text-ink' : 'text-ink-2'}" data-testid="action-choice-receipts" aria-pressed={copyReceipts} onclick={() => oncopy(!copyReceipts)}>
        <Icon icon={receipts.icon ?? 'lucide:receipt'} class="text-[14px] text-ink-3" />
        <span class="flex min-w-0 flex-col leading-4">
          <span class="overflow-hidden text-ellipsis whitespace-nowrap">{t('chats.actions.copyReceipts')}</span>
          <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[11.5px] text-ink-3">{t('chats.actions.copyReceiptsHint')}</span>
        </span>
        <Icon icon={copyReceipts ? 'lucide:circle-check' : 'lucide:circle'} class="text-[14px] {copyReceipts ? 'text-accent' : 'text-ink-3'}" />
      </button>
    </li>
  {/if}
  {#each attachable as action (action.id)}
    {@const on = chosen.includes(action.id)}
    <li>
      <button type="button" class="grid h-8 w-full grid-cols-[1.5rem_minmax(0,1fr)_1rem] items-center gap-1.5 rounded-[0.375rem] px-1 text-left text-[13px] hover:bg-paper-3 {on ? 'text-ink' : 'text-ink-2'} {action.enabled ? '' : 'line-through decoration-ink-3'}" data-testid="action-choice" data-id={action.id} data-key={action.key} aria-pressed={on} onclick={() => toggle(action.id)}>
        <Icon icon={action.icon ?? 'lucide:zap'} class="text-[14px] text-ink-3" />
        <span class="overflow-hidden text-ellipsis whitespace-nowrap">{action.name}</span>
        <Icon icon={on ? 'lucide:circle-check' : 'lucide:circle'} class="text-[14px] {on ? 'text-accent' : 'text-ink-3'}" />
      </button>
    </li>
  {:else}
    {#if !receipts}<li class="px-1 py-2 text-[12.5px] text-ink-3">{t('chats.actions.noneAttachable')}</li>{/if}
  {/each}
</ul>
