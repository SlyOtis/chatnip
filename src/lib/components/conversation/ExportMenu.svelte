<script lang="ts">
  import { anchored, boxOf } from '$lib/popover'
  import { exportChat, type ExportFormat } from '$lib/export.svelte'
  import { selection } from '$lib/reply.svelte'
  import { Button, IconButton, MenuItem } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  let { labelled = false }: { labelled?: boolean } = $props()

  let anchor = $state<HTMLDivElement>()
  let open = $state(false)
  let busy = $state(false)

  const label = $derived(selection.ids.length ? t('conversation.export.selected', { count: selection.ids.length }) : t('conversation.export.chat'))

  async function pick(format: ExportFormat) {
    open = false
    busy = true
    try {
      await exportChat(format)
    } finally {
      busy = false
    }
  }
</script>

<div class="relative" bind:this={anchor}>
  {#if labelled}
    <Button testid="chat-export" size="sm" icon="lucide:file-down" {busy} onclick={() => (open = !open)}>{label}</Button>
  {:else}
    <IconButton testid="chat-export" icon="lucide:file-down" {label} active={open} {busy} onclick={() => (open = !open)} />
  {/if}
  {#if open}
    <div class="menu w-48" use:anchored={{ at: () => boxOf(anchor ?? null), align: 'end', onclose: () => (open = false) }} data-testid="chat-export-menu" role="menu">
      <p class="menu-label">{label}</p>
      <MenuItem testid="chat-export-markdown" icon="lucide:file-text" onclick={() => pick('markdown')}>{t('conversation.export.markdown')}</MenuItem>
      <MenuItem testid="chat-export-pdf" icon="lucide:file-type" onclick={() => pick('pdf')}>{t('conversation.export.pdf')}</MenuItem>
    </div>
  {/if}
</div>
