<script lang="ts">
  import Icon from '@iconify/svelte'
  import { modalsStore, modalsClose } from '$lib/modals'
  import { Button, Sheet } from '$lib/components/ui'
  import { api } from '$lib/tauri'
  import type { ContentItem } from '$lib/tauri/types'
  import { contentSrc } from '$lib/media'
  import { imageData } from '$lib/avatar'
  import { t } from '$lib/i18n.svelte'

  const request = $derived($modalsStore.imageUpload)
  let library = $state<ContentItem[]>([])
  let failure = $state<string | null>(null)
  let busy = $state(false)

  $effect(() => {
    if (!request?.library) return
    api
      .getContent('image')
      .then((items) => (library = items.filter((item) => item.mime_type.startsWith('image/'))))
      .catch((error) => (failure = String(error)))
  })

  function deliver(image: string | null) {
    const pick = request?.onpick
    if (pick) {
      pick(image)
      modalsClose()
      return
    }
    if (image)
      modalsStore.update((state) => ({
        ...state,
        imageUpload: { ...state.imageUpload, newImage: image },
        isOpen: false
      }))
  }

  async function take(source: Blob | string) {
    busy = true
    failure = null
    try {
      deliver(request?.onpick ? await imageData(source) : await read(source))
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  function read(source: Blob | string): Promise<string> {
    if (typeof source === 'string') return Promise.resolve(source)
    return new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = (e) => (typeof e.target?.result === 'string' ? resolve(e.target.result) : reject(new Error(t('modals.avatar.unreadable'))))
      reader.onerror = () => reject(new Error(t('modals.avatar.unreadable')))
      reader.readAsDataURL(source)
    })
  }

  function handleImageUpload(event: Event) {
    const file = (event.target as HTMLInputElement).files?.[0]
    if (file) take(file)
  }
</script>

<Sheet label={request?.title ?? t('modals.avatar.title')} width="30rem" onclose={modalsClose}>
  <div class="px-5 pb-4 pt-5">
    <h2 class="text-[15px] font-semibold tracking-tight text-ink">{request?.title ?? t('modals.avatar.title')}</h2>
    <p class="mt-0.5 text-[13.5px] text-ink-3">{t('modals.avatar.formats')}</p>
    <label for="imageUpload" class="mt-4 flex cursor-pointer flex-col items-center gap-2 rounded-[0.625rem] border border-dashed border-line-strong px-6 py-9 text-center hover:border-accent hover:bg-accent-soft" aria-busy={busy}>
      <Icon icon={busy ? 'lucide:loader-circle' : 'lucide:upload'} class="text-[22px] text-ink-3 {busy ? 'animate-spin' : ''}" />
      <span class="text-[14px] font-medium text-ink">{t('modals.avatar.choose')}</span>
      <input type="file" accept="image/*,.gif" onchange={handleImageUpload} class="hidden" id="imageUpload" />
    </label>
    {#if request?.library}
      <p class="mt-4 flex items-center gap-1.5 text-[12.5px] text-ink-3"><Icon icon="lucide:library" class="text-[13px]" />{t('modals.avatar.fromContent')}</p>
      {#if library.length}
        <div class="mt-1.5 grid max-h-[14rem] grid-cols-[repeat(auto-fill,minmax(min(100%,4.5rem),1fr))] gap-1.5 overflow-y-auto">
          {#each library as item (item.id)}
            <button type="button" data-testid="image-library-item" data-id={item.id} class="aspect-square overflow-hidden rounded-[0.5rem] bg-paper-2 hover:ring-2 hover:ring-accent disabled:opacity-40" title={item.name} aria-label={item.name} disabled={busy} onclick={() => take(contentSrc(item))}>
              <img src={contentSrc(item)} alt="" class="size-full object-cover" loading="lazy" />
            </button>
          {/each}
        </div>
      {:else}
        <p class="mt-1.5 text-[12.5px] text-ink-3">{t('modals.avatar.noContent')}</p>
      {/if}
    {/if}
    {#if failure}
      <p data-testid="image-upload-error" class="mt-3 text-[12.5px] text-error-600">{failure}</p>
    {/if}
  </div>
  {#snippet footer()}
    {#if request?.resettable}
      <Button icon="lucide:rotate-ccw" onclick={() => deliver(null)}>{t('modals.avatar.reset')}</Button>
    {/if}
    <Button class="ml-auto" onclick={modalsClose}>{t('common.cancel')}</Button>
  {/snippet}
</Sheet>
