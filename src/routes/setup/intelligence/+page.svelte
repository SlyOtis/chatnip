<script lang="ts">
  import { goto } from '$app/navigation'
  import type { IntelligenceConnection } from '$lib/tauri/types'
  import { providersStore } from '$lib/tauri'
  import AiSetup from '$lib/components/settings/AiSetup.svelte'
  import { Button } from '$lib/components/ui'
  import { MAIN_ROUTE, SETUP_PROVIDERS, setupReady } from '$lib/startup'
  import { t } from '$lib/i18n.svelte'

  let connections = $state<IntelligenceConnection[]>([])
  const ready = $derived(setupReady($providersStore?.data || []))

  function finish() {
    goto(ready ? MAIN_ROUTE : SETUP_PROVIDERS)
  }
</script>

<div class="mx-auto flex h-full max-w-[44rem] flex-col justify-center px-10 py-8">
  <h1 class="text-[28px] font-semibold leading-tight tracking-tight text-ink">{t('setup.intelligence.title')}</h1>
  <p class="mt-2 max-w-[60ch] text-[14px] text-ink-2">
    {t('setup.intelligence.intro')}
  </p>

  <div class="mt-8">
    <AiSetup onconnections={(next) => (connections = next)} />
  </div>

  <div class="mt-8 flex items-center gap-3">
    <Button variant="primary" keys="↵" testid="setup-finish" onclick={finish}>{ready ? t('setup.intelligence.open') : t('setup.intelligence.back')}</Button>
    <span class="font-mono text-[11px] text-ink-3">{t('setup.step', { step: 3, total: 3 })} · {ready ? t('setup.intelligence.origins', { count: connections.length }) : t('setup.providers.needsAccount')}</span>
  </div>
</div>
