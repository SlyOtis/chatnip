<script lang="ts">
  import { Field } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  let { profileName = $bindable() }: { profileName: string } = $props()
</script>

<label class="flex flex-col gap-1.5">
  <span class="text-[13px] font-medium text-ink-2">{t('common.name')}</span>
  <Field bind:value={profileName} type="text" placeholder={t('setup.profile.namePlaceholder')} autocomplete="name" />
</label>
