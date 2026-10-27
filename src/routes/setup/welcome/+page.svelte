<script lang="ts">
  import { goto } from '$app/navigation'
  import { Button } from '$lib/components/ui'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  const points: { title: MessageKey; text: MessageKey }[] = [
    { title: 'setup.welcome.inboxTitle', text: 'setup.welcome.inboxText' },
    { title: 'setup.welcome.sortedTitle', text: 'setup.welcome.sortedText' },
    { title: 'setup.welcome.calendarTitle', text: 'setup.welcome.calendarText' }
  ]
</script>

<div class="mx-auto flex h-full max-w-[40rem] flex-col justify-center px-10 py-8">
  <h1 class="text-[28px] font-semibold leading-tight tracking-tight text-ink">{t('setup.welcome.title')}</h1>
  <p class="mt-2 text-[14px] text-ink-2">{t('setup.welcome.intro')}</p>

  <dl class="mt-8 divide-y divide-line">
    {#each points as point (point.title)}
      <div class="grid grid-cols-[11rem_minmax(0,1fr)] gap-4 py-3">
        <dt class="text-[14px] font-semibold text-ink">{t(point.title)}</dt>
        <dd class="text-[14px] leading-6 text-ink-2">{t(point.text)}</dd>
      </div>
    {/each}
  </dl>

  <div class="mt-8 flex items-center gap-3">
    <Button variant="primary" keys="↵" testid="setup-start" onclick={() => goto('/setup/providers')}>{t('setup.welcome.start')}</Button>
    <span class="font-mono text-[11px] text-ink-3">{t('setup.step', { step: 1, total: 3 })}</span>
  </div>
</div>

<svelte:window onkeydown={(e) => e.key === 'Enter' && goto('/setup/providers')} />
