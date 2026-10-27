<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri'
  import type { AssistantRun } from '$lib/tauri/types'
  import { formatCost, formatDuration } from '$lib/intelligence'
  import { Field, Row, Section, Segmented, Switch } from '$lib/components/ui'
  import { formatDate, t } from '$lib/i18n.svelte'
  import IntelligenceConnections from './IntelligenceConnections.svelte'
  import SettingFields from './SettingFields.svelte'
  import type { SettingField } from '$lib/settings'

  let settings = $state<Record<string, string>>({})
  let runs = $state<(AssistantRun & { origin?: string })[]>([])
  let failure = $state<string | null>(null)

  const enabled = $derived((settings['assistant.enabled'] ?? 'true') !== 'false')

  const fields: SettingField[] = [
    { key: 'assistant.model', anchor: 'ai.claude-model', label: 'settings.ai.claudeModel', hint: 'settings.ai.claudeModelHint' },
    { key: 'ollama.host', anchor: 'ai.ollama-host', label: 'settings.ai.ollamaHost', hint: 'settings.ai.ollamaHostHint' }
  ]

  onMount(async () => {
    const results = await Promise.allSettled([api.getAppSettings(), api.getAssistantRuns(20)])
    if (results[0].status === 'fulfilled') settings = results[0].value
    if (results[1].status === 'fulfilled') runs = results[1].value
    const first = results.find((r) => r.status === 'rejected')
    if (first && first.status === 'rejected') failure = String(first.reason)
  })

  async function put(key: string, value: string) {
    if ((settings[key] ?? '') === value) return
    try {
      settings = await api.putAppSetting(key, value)
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  function when(iso: string): string {
    return formatDate(iso, { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' })
  }
</script>

<div>
  <Section title={t('settings.tab.assistant')} description={t('settings.assistant.description')}>
    {#if failure}
      <p class="mb-2 text-[13px] text-error-500">{failure}</p>
    {/if}
    <Row id="ai.enabled" label={t('settings.assistant.use')} hint={t('settings.assistant.useHint')}>
      <Switch label={t('settings.assistant.use')} checked={enabled} onchange={(on) => put('assistant.enabled', on ? 'true' : 'false')} />
    </Row>
  </Section>

  <IntelligenceConnections />

  <div class="pb-4">
    <SettingFields {fields} />
  </div>

  <Section title={t('settings.summaries.title')} description={t('settings.summaries.description')}>
    <Row id="ai.summaries" label={t('settings.summaries.length')} hint={t('settings.summaries.lengthHint')}>
      <Segmented size="sm" testid="summaries-length" label={t('settings.summaries.length')} value={settings['summaries.length'] || 'medium'} options={[{ value: 'short', label: t('settings.summaries.short') }, { value: 'medium', label: t('settings.summaries.medium') }, { value: 'long', label: t('settings.summaries.long') }]} onchange={(v) => put('summaries.length', v)} />
    </Row>
    <Row label={t('settings.summaries.form')} hint={t('settings.summaries.formHint')}>
      <Segmented size="sm" testid="summaries-form" label={t('settings.summaries.form')} value={settings['summaries.form'] || 'prose'} options={[{ value: 'prose', label: t('settings.summaries.prose'), icon: 'lucide:align-left' }, { value: 'bullets', label: t('settings.summaries.bullets'), icon: 'lucide:list' }]} onchange={(v) => put('summaries.form', v)} />
    </Row>
    <Row label={t('settings.summaries.language')} hint={t('settings.summaries.languageHint')}>
      <Segmented size="sm" testid="summaries-language" label={t('settings.summaries.language')} value={settings['summaries.language'] || 'conversation'} options={[{ value: 'conversation', label: t('settings.summaries.languageConversation') }, { value: 'profile', label: t('settings.summaries.languageApp') }]} onchange={(v) => put('summaries.language', v)} />
    </Row>
    <Row label={t('settings.summaries.actions')} hint={t('settings.summaries.actionsHint')}>
      <Switch testid="summaries-actions" label={t('settings.summaries.actions')} checked={settings['summaries.actions'] !== 'false'} onchange={(on) => put('summaries.actions', on ? 'true' : 'false')} />
    </Row>
    <Row label={t('settings.summaries.instructions')} hint={t('settings.summaries.instructionsHint')}>
      <Field data-testid="summaries-instructions" size="sm" value={settings['summaries.instructions'] ?? ''} placeholder={t('settings.summaries.instructionsPlaceholder')} aria-label={t('settings.summaries.instructions')} onchange={(e) => put('summaries.instructions', e.currentTarget.value.trim())} />
    </Row>
  </Section>

  <Section id="ai.runs" title={t('settings.assistant.runs')} description={t('settings.assistant.runsDescription')} total={runs.length}>
    {#snippet children(shown)}
    {#if runs.length === 0}
      <p class="py-4 text-[13px] text-ink-3">{t('settings.assistant.noRuns')}</p>
    {:else}
      <table class="w-full table-fixed text-[13px]">
        <thead>
          <tr class="h-8 text-left text-[12px] font-medium text-ink-3">
            <th class="w-[22%]">{t('settings.assistant.prompt')}</th>
            <th class="w-[20%]">{t('settings.assistant.origin')}</th>
            <th class="w-[24%]">{t('settings.assistant.model')}</th>
            <th class="w-[11%] text-right">{t('settings.assistant.duration')}</th>
            <th class="w-[9%] text-right">{t('settings.assistant.cost')}</th>
            <th class="w-[14%] text-right">{t('settings.assistant.when')}</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-line">
          {#each runs.slice(0, shown) as run (run.id)}
            <tr class="h-9 text-ink-2">
              <td class="truncate font-medium text-ink">{run.kind}</td>
              <td class="truncate">{run.origin ?? ''}</td>
              <td class="truncate font-mono text-[12px]">{run.model}</td>
              <td class="whitespace-nowrap text-right font-mono text-[12px]">{formatDuration(run.duration_ms)}</td>
              <td class="whitespace-nowrap text-right font-mono text-[12px]">{formatCost(run.cost_usd)}</td>
              <td class="whitespace-nowrap text-right font-mono text-[12px]">{when(run.created_at)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    {/snippet}
  </Section>
</div>
