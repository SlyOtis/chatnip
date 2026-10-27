<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri'
  import type { SettingRegistry } from '$lib/tauri/types'
  import type { SettingField } from '$lib/settings'
  import { Button, Field, Row } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    fields: SettingField[]
    onsaved?: () => void
  }

  let { fields, onsaved }: Props = $props()

  let registry = $state<SettingRegistry | null>(null)
  let stored = $state<Record<string, string>>({})
  let drafts = $state<Record<string, string>>({})
  let saving = $state<Record<string, boolean>>({})
  let failure = $state<string | null>(null)

  onMount(async () => {
    try {
      const [loaded, values] = await Promise.all([api.getSettingRegistry(), api.getAppSettings()])
      registry = loaded
      stored = values
      drafts = Object.fromEntries(fields.map((field) => [field.key, values[field.key] ?? '']))
    } catch (error) {
      failure = String(error)
    }
  })

  async function save(key: string) {
    saving = { ...saving, [key]: true }
    try {
      stored = await api.putAppSetting(key, drafts[key] ?? '')
      drafts = { ...drafts, [key]: stored[key] ?? '' }
      failure = null
      onsaved?.()
    } catch (error) {
      failure = String(error)
    } finally {
      saving = { ...saving, [key]: false }
    }
  }
</script>

{#each fields as field (field.key)}
  {@const fallback = registry?.defaults[field.key] ?? ''}
  <Row id={field.anchor} label={t(field.label)} hint={t(field.hint, { value: fallback })} for={field.key}>
    <Field
      id={field.key}
      data-testid="setting-field"
      data-key={field.key}
      data-ready={registry !== null}
      mono
      type="text"
      class={field.wide ? 'w-[min(26rem,40vw)]' : 'w-[min(16rem,30vw)]'}
      bind:value={() => drafts[field.key] ?? '', (value) => (drafts[field.key] = value)}
      placeholder={fallback}
      autocomplete="off"
      spellcheck="false"
      onkeydown={(e) => e.key === 'Enter' && save(field.key)}
    />
    <Button size="sm" variant="secondary" busy={saving[field.key]} disabled={(drafts[field.key] ?? '') === (stored[field.key] ?? '')} onclick={() => save(field.key)}>{t('common.save')}</Button>
  </Row>
{/each}
{#if failure}
  <p class="py-2 text-[13px] text-error-500">{failure}</p>
{/if}
