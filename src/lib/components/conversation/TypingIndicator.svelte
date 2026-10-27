<script lang="ts">
  import { get } from 'svelte/store'
  import { t } from '$lib/i18n.svelte'
  import { conversationStore, peopleStore } from '$lib/tauri'
  import { personOf } from '$lib/tauri/people'
  import { typingStore } from '$lib/tauri/presence'
  import { appearanceSettings, globalAppearance, resolved, type Presence } from '$lib/appearance'

  let { conversationId = null }: { conversationId?: string | null } = $props()

  $effect(() => {
    const interval = setInterval(() => {
      const current = get(typingStore)
      const now = Date.now()
      let changed = false
      const updated = new Map(current)
      for (const [key, value] of updated) {
        if (now - value.timestamp > 5000) {
          updated.delete(key)
          changed = true
        }
      }
      if (changed) typingStore.set(updated)
    }, 1000)
    return () => clearInterval(interval)
  })

  function presenceOf(userId: string): Presence {
    const conversation = $conversationStore.data?.id ?? null
    const person = personOf($peopleStore, { user_id: userId })
    return resolved($appearanceSettings, $globalAppearance, conversation, person?.id ?? null).presence
  }

  let open = $derived(conversationId ?? $conversationStore.data?.id ?? null)
  let typers = $derived(Array.from($typingStore.values()).filter((value) => value.conversation_id === open).map((value) => ({ name: value.user_name, presence: presenceOf(value.user_id ?? '') })).filter((typer) => typer.presence !== 'none'))
  let presence = $derived(typers[0]?.presence ?? 'dots')

  let typingText = $derived.by(() => {
    if (typers.length === 0) return ''
    if (typers.length === 1) return t('conversation.typing.one', { name: typers[0].name })
    if (typers.length === 2) return t('conversation.typing.two', { first: typers[0].name, second: typers[1].name })
    return t('conversation.typing.several')
  })
</script>

{#if typers.length > 0}
  <div class="flex items-center gap-2 px-8 py-1 text-[13px] text-ink-3" data-testid="typing-indicator" data-presence={presence}>
    {#if presence === 'pulse'}
      <span class="presence-pulse size-2 rounded-full bg-accent"></span>
    {:else}
      <span class="flex items-center gap-0.5">
        <span class="presence-{presence} size-1 rounded-full bg-ink-3"></span>
        <span class="presence-{presence} size-1 rounded-full bg-ink-3 [animation-delay:150ms]"></span>
        <span class="presence-{presence} size-1 rounded-full bg-ink-3 [animation-delay:300ms]"></span>
      </span>
    {/if}
    <span>{typingText}</span>
  </div>
{/if}

<style>
  .presence-dots {
    animation: typing-pulse 1.4s infinite ease-in-out;
  }
  .presence-wave {
    animation: typing-wave 1.2s infinite ease-in-out;
  }
  .presence-pulse {
    animation: typing-ring 1.6s infinite ease-out;
  }
  @keyframes typing-pulse {
    0%, 60%, 100% { opacity: 0.3; transform: scale(1); }
    30% { opacity: 1; transform: scale(1.3); }
  }
  @keyframes typing-wave {
    0%, 60%, 100% { transform: translateY(0); }
    30% { transform: translateY(-0.25em); }
  }
  @keyframes typing-ring {
    0% { box-shadow: 0 0 0 0 color-mix(in oklab, var(--accent) 45%, transparent); }
    100% { box-shadow: 0 0 0 0.75em transparent; }
  }
</style>
