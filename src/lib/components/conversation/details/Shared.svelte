<script lang="ts">
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import { listen } from '@tauri-apps/api/event'
  import { convertFileSrc } from '@tauri-apps/api/core'
  import Icon from '@iconify/svelte'
  import { api, conversationsById, peopleStore } from '$lib/tauri'
  import type { GetSharedInput, SharedFile, SharedLink } from '$lib/tauri/types'
  import { rowModel } from '$lib/inbox'
  import { myUserIds } from '$lib/tauri/people'
  import { messageHref, openLink, revealMessage } from '$lib/messageLink'
  import { modalsShow } from '$lib/modals'
  import { showTooltip } from '$lib/tooltip'
  import { IconButton } from '$lib/components/ui'
  import { getAttachmentIcon } from '../messages/AttachmentPreview.svelte'
  import { formatSize } from '$lib/links'
  import { formatDate, t } from '$lib/i18n.svelte'

  interface Props {
    scope: GetSharedInput
    grouped?: boolean
  }

  let { scope, grouped = false }: Props = $props()

  const SHOWN = 8

  let files = $state<SharedFile[]>([])
  let links = $state<SharedLink[]>([])
  let allFiles = $state(false)
  let allLinks = $state(false)
  let opening = $state<string[]>([])
  let failure = $state<string | null>(null)

  const shownFiles = $derived(allFiles ? files : files.slice(0, SHOWN))
  const shownLinks = $derived(allLinks ? links : links.slice(0, SHOWN))

  async function load() {
    try {
      const [loadedFiles, loadedLinks] = await Promise.all([api.getSharedFiles(scope), api.getSharedLinks(scope)])
      files = loadedFiles
      links = loadedLinks
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  onMount(() => {
    load()
    const off = listen('messages-updated', () => load())
    return () => off.then((stop) => stop())
  })

  function chatName(item: SharedFile | SharedLink): string {
    const conversation = $conversationsById.get(item.conversation_id)
    return conversation ? rowModel({ kind: 'conversation', conversation }, $peopleStore, $myUserIds).title : item.conversation_name
  }

  function origin(item: SharedFile | SharedLink): string {
    const sender = item.sender || t('common.unknown')
    return grouped ? t('conversation.shared.from', { sender, chat: chatName(item) }) : sender
  }

  const day = (at: string) => formatDate(at, { month: 'short', day: 'numeric' })
  const isMedia = (file: SharedFile) => file.content_type.startsWith('image/') || file.content_type.startsWith('video/')

  async function openFile(file: SharedFile, quick_look: boolean) {
    if (isMedia(file)) {
      modalsShow({ source: file.url ?? `asset://localhost/${file.attachment_id}`, alt: file.name })
      return
    }
    if (opening.includes(file.id)) return
    opening = [...opening, file.id]
    failure = null
    try {
      await api.postAttachmentOpen({ message_id: file.message_id, attachment_id: file.attachment_id, quick_look })
    } catch (error) {
      failure = String(error)
    } finally {
      opening = opening.filter((id) => id !== file.id)
    }
  }

  async function jump(item: SharedFile | SharedLink) {
    if (!grouped && page.url.pathname.startsWith('/main/conversation/') && (await revealMessage(item.message_id, { delay: 0 }).done)) return
    await goto(messageHref(item.conversation_id, item.message_id))
  }
</script>

{#snippet toggle(expanded: boolean, count: number, onclick: () => void, testid: string)}
  {#if count > SHOWN}
    <button type="button" data-testid={testid} class="mt-1 text-[12.5px] text-ink-3 hover:text-ink" {onclick}>
      {expanded ? t('common.showLess') : t('conversation.shared.showAll', { count })}
    </button>
  {/if}
{/snippet}

{#if files.length}
  <section data-testid="shared-files" class="border-b border-line py-3">
    <p class="text-[13px] text-ink-3">{t('conversation.shared.files')} <span class="font-mono text-[11px]">{files.length}</span></p>
    <div class="flex flex-col pt-1.5">
      {#each shownFiles as file (file.id)}
        {@const busy = opening.includes(file.id)}
        <div data-testid="shared-file" data-id={file.id} class="group flex items-center gap-1">
          <button
            type="button"
            data-testid="shared-file-open"
            class="flex min-w-0 flex-1 items-center gap-2 rounded-[0.4rem] py-1.5 text-left"
            onmouseenter={(e) => !isMedia(file) && showTooltip(e.currentTarget, t('conversation.attachment.preview'))}
            onclick={(e) => openFile(file, !e.metaKey)}
          >
            <Icon icon={busy ? 'lucide:loader-circle' : getAttachmentIcon(file.content_type)} class="shrink-0 text-[14px] text-ink-3 {busy ? 'animate-spin' : ''}" />
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[13px] text-ink group-hover:underline">{file.name}</span>
              <span class="block truncate text-[11.5px] text-ink-3">{origin(file)}</span>
            </span>
            <span class="shrink-0 text-right font-mono text-[11px] text-ink-3">{formatSize(file.size)}<br />{day(file.at)}</span>
          </button>
          <IconButton testid="shared-file-jump" icon="lucide:corner-down-right" label={t('conversation.shared.jump')} size="sm" class="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" onclick={() => jump(file)} />
        </div>
      {/each}
    </div>
    {@render toggle(allFiles, files.length, () => (allFiles = !allFiles), 'shared-files-all')}
  </section>
{/if}

{#if links.length}
  <section data-testid="shared-links" class="border-b border-line py-3">
    <p class="text-[13px] text-ink-3">{t('conversation.shared.links')} <span class="font-mono text-[11px]">{links.length}</span></p>
    <div class="flex flex-col pt-1.5">
      {#each shownLinks as link (link.id)}
        <div data-testid="shared-link" data-id={link.id} class="group flex items-center gap-1">
          <button
            type="button"
            data-testid="shared-link-open"
            class="flex min-w-0 flex-1 items-center gap-2 rounded-[0.4rem] py-1.5 text-left"
            onmouseenter={(e) => showTooltip(e.currentTarget, link.url)}
            onclick={() => openLink(link.url).catch((error) => (failure = String(error)))}
          >
            {#if link.favicon}
              <img src={convertFileSrc(link.favicon)} alt="" class="size-[14px] shrink-0 rounded-[0.2rem] object-contain" />
            {:else}
              <Icon icon="lucide:globe" class="shrink-0 text-[14px] text-ink-3" />
            {/if}
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[13px] text-ink group-hover:underline">{link.title ?? link.domain ?? link.url}</span>
              <span class="block truncate text-[11.5px] text-ink-3">{link.site_name ?? link.domain ?? link.url} · {origin(link)}</span>
            </span>
            <span class="shrink-0 font-mono text-[11px] text-ink-3">{day(link.at)}</span>
          </button>
          <IconButton testid="shared-link-jump" icon="lucide:corner-down-right" label={t('conversation.shared.jump')} size="sm" class="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" onclick={() => jump(link)} />
        </div>
      {/each}
    </div>
    {@render toggle(allLinks, links.length, () => (allLinks = !allLinks), 'shared-links-all')}
  </section>
{/if}

{#if failure && (files.length || links.length)}
  <p data-testid="shared-error" class="pt-1 text-[12px] text-error-500">{failure}</p>
{/if}
