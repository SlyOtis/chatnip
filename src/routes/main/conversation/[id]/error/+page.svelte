<script lang="ts">
  import { page } from '$app/state';
  import { t } from '$lib/i18n.svelte';
  let conversationId = $derived(page.params.id);
</script>

<div class="flex h-full items-center justify-center bg-paper">
  <div class="flex max-w-md flex-col items-center gap-3 px-6 text-center">
    <p class="text-[15px] font-semibold tracking-tight text-ink">{t('conversation.error.title')}</p>
    <p class="text-[13px] text-ink-3">{t('conversation.error.hint')}</p>
    <code class="rounded-md bg-paper-2 px-2 py-0.5 font-mono text-[11px] text-ink-2 hairline">{conversationId}</code>
    <button class="mt-2 rounded-[var(--radius-row)] bg-paper-2 px-3 py-1.5 text-[13px] font-medium text-ink hover:bg-paper-3 hairline" onclick={() => window.history.back()}>
      {t('conversation.error.back')}
    </button>
  </div>
</div>
