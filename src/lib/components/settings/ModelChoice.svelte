<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri'
  import type { IntelligenceConnection } from '$lib/tauri/types'
  import { featureKey, featureSelection, modelChoices, modelPicker, parseModelChoice, type ModelDefaults, type ModelSelection } from '$lib/intelligence'
  import { IconButton, Select } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    label: string
    testid: string
    feature?: string
    selection?: ModelSelection
    defaults?: ModelDefaults
    class?: string
    onchange?: (selection: ModelSelection) => void | Promise<void>
  }

  let { label, testid, feature, selection, defaults, class: className = 'w-[min(22rem,44vw)]', onchange }: Props = $props()

  let connections = $state<IntelligenceConnection[]>([])
  let claude = $state<string | null>(null)
  let values = $state<Record<string, string>>({})
  let presets = $state<ModelDefaults['models']>([])
  let opened = $state(false)
  let failure = $state<string | null>(null)

  const chosen = $derived(selection ?? (feature ? featureSelection(feature, values) : { connection: '', model: '' }))
  const fallback = $derived(defaults ?? { models: presets, local: false })
  const choices = $derived(modelChoices(connections, claude))
  const picker = $derived(modelPicker(chosen, connections, fallback, claude))

  onMount(async () => {
    const results = await Promise.allSettled([api.getIntelligenceConnections(), api.getAiSetup(false), feature ? api.getSettingRegistry() : Promise.resolve(null)])
    if (results[0].status === 'fulfilled') connections = results[0].value
    if (results[1].status === 'fulfilled') claude = results[1].value.claude.model
    if (results[2].status === 'fulfilled' && results[2].value) {
      values = results[2].value.values
      presets = results[2].value.features.find((f) => f.id === feature)?.models ?? []
    }
    const first = results.find((r) => r.status === 'rejected')
    if (first && first.status === 'rejected') failure = String(first.reason)
  })

  async function choose(value: string) {
    const next = value ? parseModelChoice(value) : { connection: '', model: '' }
    try {
      if (feature) {
        values = await api.putAppSetting(featureKey(feature, 'connection'), next.connection)
        values = await api.putAppSetting(featureKey(feature, 'model'), next.model)
      }
      await onchange?.(next)
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }
</script>

<span class="flex min-w-0 items-center gap-1.5">
  {#if picker.missing}
    <IconButton size="sm" tone="warn" icon="lucide:triangle-alert" testid="model-missing-{testid}" label={t('settings.models.missing', { choice: picker.label })} onclick={() => (opened = true)} />
  {/if}
  <Select
    size="sm"
    class={className}
    testid="model-choice-{testid}"
    dialog={label}
    muted={!picker.own}
    placeholder={picker.label}
    bind:open={opened}
    value={picker.value}
    options={[{ value: '', label: picker.defaultLabel }, ...choices]}
    onchange={choose}
  />
  {#if picker.own}
    <IconButton size="sm" icon="lucide:rotate-ccw" testid="model-reset-{testid}" label={t('settings.models.reset')} onclick={() => choose('')} />
  {/if}
</span>
{#if failure}
  <p class="text-[12px] text-error-500">{failure}</p>
{/if}
