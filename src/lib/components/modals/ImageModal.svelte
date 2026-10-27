<script lang="ts">
  import { modalsStore, modalsClose } from '$lib/modals'
  import { Button, Kbd, Sheet } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'


  function saveImage() {
    if ($modalsStore.image?.source) {
      const link = document.createElement('a')
      link.href = $modalsStore.image.source
      link.download = 'image'
      link.click()
      modalsClose()
    }
  }
</script>

{#if $modalsStore.image?.source}
  <Sheet label={t('modals.image.title')} width="min(90vw, 70rem)" onclose={modalsClose}>
    <div class="flex items-center justify-center p-3">
      <img src={$modalsStore.image.source} alt={$modalsStore.image.alt || t('modals.image.enlarged')} class="max-h-[70vh] w-auto max-w-full object-contain" />
    </div>
    {#snippet footer()}
      <Button icon="lucide:download" onclick={saveImage}>{t('common.save')}</Button>
      <span class="ml-auto flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="esc" /> {t('modals.footer.close')}</span>
    {/snippet}
  </Sheet>
{/if}
