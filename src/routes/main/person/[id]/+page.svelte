<script lang="ts">
  import { goto } from '$app/navigation'
  import PersonThread from '$lib/components/conversation/PersonThread.svelte'
  import { conversationStore } from '$lib/tauri/conversations'
  import { openPerson, peopleStore } from '$lib/tauri/people'
  import { loadChats } from '$lib/tauri/chats'
  import { appearanceSettings, globalAppearance, conversationKey, personKey, scoped, scopePresentation } from '$lib/appearance'
  import { themeChoice, resolvedTheme } from '$lib/theme-mode'
  import type { PageProps } from './$types'

  let { data }: PageProps = $props()

  let redirected = $state<string | null>(null)

  const keys = $derived([personKey(data.personId), ...($conversationStore.data?.id ? [conversationKey($conversationStore.data.id)] : [])])
  const scope = $derived(scopePresentation($globalAppearance, scoped($appearanceSettings, keys), resolvedTheme($themeChoice)))

  $effect(() => {
    const id = decodeURIComponent(data.personId)
    const person = $peopleStore.find((candidate) => candidate.id === id)
    if (!person || person.conversation_ids.length > 0 || redirected === id) return
    redirected = id
    loadChats()
      .then((chats) => openPerson(person, chats, (path) => goto(path, { replaceState: true })))
      .catch(() => {})
  })
</script>

<div class="relative isolate h-full overflow-hidden {scope.tinted ? `frost-${scope.tint}` : ''}" style={scope.style} style:--frost-alpha={scope.solid ? '1' : undefined}>
  {#if scope.backdrop}<div class="scope-backdrop" style:background={scope.backdrop}></div>{/if}
  {#if scope.tinted}<div class="scope-frost"></div>{/if}
  {#key data.personId}
    <PersonThread personId={data.personId} />
  {/key}
</div>
