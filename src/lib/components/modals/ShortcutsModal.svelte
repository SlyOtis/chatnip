<script lang="ts">
  import { modalsClose } from '$lib/modals'
  import { COMPOSER_CATEGORY, composerKeys, getShortcutsByCategory, formatShortcutKeys } from '$lib/shortcuts'
  import { layoutSettings, messageSetting } from '$lib/layoutSettings'
  import { Kbd, Sheet } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'


  let categories = $state(getShortcutsByCategory())
  const composing = $derived(composerKeys(messageSetting($layoutSettings, 'messages.enter_sends')))
</script>

<Sheet label={t('modals.command.shortcuts')} width="34rem" onclose={modalsClose}>
  <div class="px-5 pb-3 pt-5">
    <h2 class="text-[15px] font-semibold tracking-tight text-ink">{t('modals.command.shortcuts')}</h2>
  </div>
  <div class="px-2.5 pb-3">
    {#each [...categories.entries()] as [category, shortcuts] (category)}
      <div class="px-2.5 pb-1 pt-3 text-[13px] text-ink-3">{t(category)}</div>
      {#each shortcuts as shortcut (shortcut.description + shortcut.key)}
        <div class="flex h-8 items-center justify-between rounded-[0.5rem] px-2.5 hover:bg-paper-3">
          <span class="text-[14px] text-ink">{t(shortcut.description)}</span>
          <Kbd keys={formatShortcutKeys(shortcut).join(' ')} />
        </div>
      {/each}
    {/each}
    <div class="px-2.5 pb-1 pt-3 text-[13px] text-ink-3">{t(COMPOSER_CATEGORY)}</div>
    {#each composing as entry (entry.description)}
      <div class="flex h-8 items-center justify-between rounded-[0.5rem] px-2.5 hover:bg-paper-3">
        <span class="text-[14px] text-ink">{t(entry.description)}</span>
        <Kbd keys={entry.keys.join(' ')} />
      </div>
    {/each}
  </div>
  {#snippet footer()}
    <span class="ml-auto flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="esc" /> {t('modals.footer.close')}</span>
  {/snippet}
</Sheet>
