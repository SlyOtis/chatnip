<script lang="ts">
  import Icon from '@iconify/svelte'
  import { modalsShow, modalsStore } from '$lib/modals'
  import { t } from '$lib/i18n.svelte'

  let { profileImage = $bindable() }: { profileImage: string | null | undefined } = $props()

  function openImageUpload() {
    modalsShow({ currentImage: profileImage })
  }

  let _profileImage = $derived($modalsStore?.imageUpload?.newImage)

  $effect(() => {
    if (_profileImage) profileImage = _profileImage
  })
</script>

<div class="flex items-center gap-4">
  <button onclick={openImageUpload} type="button" class="relative flex size-12 items-center justify-center overflow-hidden rounded-full bg-paper-3 text-ink-2 hover:bg-paper-2" aria-label={t('setup.profile.choosePhoto')}>
    {#if profileImage}
      <img src={profileImage} alt="" class="size-full object-cover" />
    {:else}
      <Icon icon="lucide:user" class="text-[22px]" />
    {/if}
  </button>
  <button onclick={openImageUpload} type="button" class="text-[13px] font-medium text-ink-2 hover:text-ink">{profileImage ? t('setup.profile.changePhoto') : t('setup.profile.addPhoto')}</button>
</div>
