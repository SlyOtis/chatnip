<script lang="ts">
  import { onMount } from 'svelte'
  import { Button, Row, Section, Segmented, Switch } from '$lib/components/ui'
  import { formatBytes, pullPercent } from '$lib/intelligence'
  import { formatHotkey, HOTKEYS, voice } from '$lib/voice.svelte'
  import { t } from '$lib/i18n.svelte'
  import SettingFields from './SettingFields.svelte'
  import ModelChoice from './ModelChoice.svelte'
  import type { SettingField } from '$lib/settings'

  const fields: SettingField[] = [
    { key: 'voice.debounce_ms', anchor: 'voice.correction-delay', label: 'settings.voice.correctionDelay', hint: 'settings.voice.correctionDelayHint' },
    { key: 'voice.model_url', anchor: 'voice.model-url', label: 'settings.voice.modelUrl', hint: 'settings.voice.modelUrlHint', wide: true },
    { key: 'voice.vad_url', anchor: 'voice.vad-url', label: 'settings.voice.vadUrl', hint: 'settings.voice.vadUrlHint', wide: true }
  ]

  let busy = $state(false)
  let failure = $state<string | null>(null)

  const setup = $derived(voice.setup)
  const settings = $derived(setup?.settings)
  const present = $derived(!!setup && setup.models.every((m) => m.present))
  const bytes = $derived(setup?.models.reduce((sum, m) => sum + (m.present ? m.bytes : 0), 0) ?? 0)
  const downloads = $derived(Object.values(voice.progress).filter((p) => !p.done))

  const languages = $derived([
    { value: 'auto', label: t('settings.voice.languageAuto') },
    { value: 'no', label: t('settings.voice.languageNo') },
    { value: 'en', label: t('settings.voice.languageEn') }
  ])
  const cleanups = $derived([
    { value: 'live', label: t('settings.voice.cleanupLive') },
    { value: 'stop', label: t('settings.voice.cleanupStop') },
    { value: 'off', label: t('settings.voice.cleanupOff') }
  ])
  const hotkeys = $derived(HOTKEYS.map((value) => ({ value, label: value === 'off' ? t('settings.voice.hotkeyOff') : formatHotkey(value), mono: value !== 'off' })))

  onMount(() => {
    voice.load().catch((error) => (failure = String(error)))
  })

  async function run(action: () => Promise<unknown>) {
    busy = true
    try {
      await action()
      failure = null
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }
</script>

<Section title={t('settings.voice.title')} description={t('settings.voice.description')}>
  <Row id="voice.enabled" label={t('settings.voice.enabled')} hint={t('settings.voice.enabledHint')}>
    <Switch label={t('settings.voice.enabled')} checked={settings?.enabled ?? false} disabled={busy || !setup} onchange={(on) => run(() => voice.put('voice.enabled', on ? 'true' : 'false'))} />
  </Row>
  <Row id="voice.models" label={t('settings.voice.models')} hint={t('settings.voice.modelsHint')}>
    {#if downloads.length}
      <div class="flex min-w-0 flex-col gap-1">
        {#each downloads as progress (progress.model)}
          {@const percent = pullPercent(progress) ?? 0}
          <span class="flex items-center gap-2">
            <span class="h-1 w-[min(10rem,30vw)] overflow-hidden rounded-full bg-paper-3"><span class="block h-full bg-accent" style:width="{percent}%"></span></span>
            <span class="font-mono text-[10.5px] text-ink-3">{t('settings.voice.downloading', { name: progress.model, percent })}</span>
          </span>
        {/each}
      </div>
    {:else}
      <span data-testid="voice-models-state" data-present={present} class="text-[12.5px] {present ? 'text-ink-2' : 'text-ink-3'}">{present ? `${t('settings.voice.ready')} · ${formatBytes(bytes)}` : t('settings.voice.missing')}</span>
      <Button size="sm" testid="voice-setup" icon="lucide:download" disabled={busy || voice.settingUp} onclick={() => run(() => voice.setUp(present))}>{present ? t('settings.voice.setUpAgain') : t('settings.voice.setUp')}</Button>
    {/if}
  </Row>
  <Row id="voice.language" label={t('settings.voice.language')} hint={t('settings.voice.languageHint')}>
    <Segmented label={t('settings.voice.language')} size="sm" value={settings?.language ?? 'auto'} options={languages} onchange={(value) => run(() => voice.put('voice.language', value))} />
  </Row>
  <Row id="voice.cleanup" label={t('settings.voice.cleanup')} hint={setup && !setup.cleanup_available ? t('settings.voice.cleanupMissing') : t('settings.voice.cleanupHint')}>
    <Segmented label={t('settings.voice.cleanup')} size="sm" value={settings?.cleanup ?? 'live'} options={cleanups} onchange={(value) => run(() => voice.put('voice.cleanup', value))} />
  </Row>
  <Row id="voice.model-voice" label={t('settings.models.feature.voice')} hint={t('settings.models.feature.voiceHint')}>
    <ModelChoice label={t('settings.models.feature.voice')} testid="voice" feature="voice" />
  </Row>
  <Row id="voice.hotkey" label={t('settings.voice.hotkey')} hint={t('settings.voice.hotkeyHint')}>
    <Segmented label={t('settings.voice.hotkey')} size="sm" value={settings?.hotkey ?? HOTKEYS[0]} options={hotkeys} onchange={(value) => run(() => voice.put('voice.hotkey', value))} />
  </Row>
  <SettingFields {fields} onsaved={() => voice.load().catch((error) => (failure = String(error)))} />
  {#if setup?.microphone === 'denied' || setup?.microphone === 'restricted' || voice.snapshot.reason === 'microphone'}
    <p class="py-2 text-[13px] text-error-600">{t('settings.voice.microphoneDenied')}</p>
  {/if}
  {#if failure}
    <p class="py-2 text-[13px] text-error-600">{failure}</p>
  {/if}
</Section>
