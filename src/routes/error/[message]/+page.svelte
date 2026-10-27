<script lang="ts">
  import { page } from '$app/state'
  import { goto } from '$app/navigation'
  import { onMount } from 'svelte'
  import { formatDate, t } from '$lib/i18n.svelte'
  import { putLaunchStep } from '$lib/window'

  onMount(() => putLaunchStep('ready'))

  const errorMessage = $derived(decodeURIComponent(page.params.message || t('app.error.unexpected')))
</script>

<div class="flex h-full items-center justify-center bg-paper p-8">
  <div class="w-full max-w-[28rem] rounded-[var(--radius-card)] bg-paper-2 p-8 hairline">
    <p class="font-mono text-[11px] uppercase tracking-wider text-error-500">{t('common.error')}</p>
    <h1 class="mt-2 text-[22px] font-semibold tracking-tight text-ink">{t('app.error.title')}</h1>
    <p class="selectable mt-3 break-words text-[14px] text-ink-2">{errorMessage}</p>
    <div class="mt-6 flex items-center gap-3">
      <button class="rounded-[var(--radius-row)] bg-accent px-4 py-2 text-[14px] font-semibold text-accent-ink hover:brightness-110" onclick={() => goto('/')}>{t('common.retry')}</button>
      <span class="font-mono text-[11px] text-ink-3">{formatDate(new Date(), { hour: '2-digit', minute: '2-digit', second: '2-digit' })}</span>
    </div>
  </div>
</div>
