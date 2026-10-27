<script lang="ts">
  import type { RuleCondition } from '$lib/tauri/types'
  import { Button, Field, IconButton, Select } from '$lib/components/ui'
  import { FIELDS, OPERATORS, emptyCondition, regexError } from './rules'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    conditions: RuleCondition[]
  }

  let { conditions = $bindable() }: Props = $props()

  const errors = $derived(conditions.map(regexError))
  const fieldOptions = $derived(FIELDS.map((f) => ({ value: f.value, label: t(f.label) })))
  const operatorOptions = $derived(OPERATORS.map((o) => ({ value: o.value, label: t(o.label) })))
</script>

<div class="flex flex-col divide-y divide-line">
  {#each conditions as condition, index (index)}
    <div class="grid grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)_minmax(0,1.6fr)_auto] items-start gap-2 py-2">
      <Select size="sm" bind:value={condition.field} options={fieldOptions} />
      <Select size="sm" bind:value={condition.operator} options={operatorOptions} />
      <div class="min-w-0">
        <Field size="sm" mono class={errors[index] ? 'ring-2 ring-error-500' : ''} placeholder={condition.operator === 'matches' ? '^(invoice|faktura)\\b' : t('settings.rules.value')} bind:value={condition.value} spellcheck="false" />
        {#if condition.operator === 'matches'}
          <p class="mt-1 text-[12px] {errors[index] ? 'text-error-500' : 'text-ink-3'}">{errors[index] ?? t('settings.rules.regexHint')}</p>
        {/if}
      </div>
      <IconButton size="sm" icon="lucide:x" label={t('settings.rules.removeCondition')} tone="danger" disabled={conditions.length === 1} onclick={() => (conditions = conditions.filter((_, i) => i !== index))} />
    </div>
  {/each}
  <div class="py-2">
    <Button size="sm" icon="lucide:plus" onclick={() => (conditions = [...conditions, emptyCondition()])}>{t('settings.rules.addCondition')}</Button>
  </div>
</div>
