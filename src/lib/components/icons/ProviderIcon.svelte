<script lang="ts">
  import Icon from '@iconify/svelte'
  import type { ConversationType } from '$lib/tauri/types'
  import { providerIcon, providerLabel, type ProviderSource } from '$lib/providerIcons'

  interface Mark extends ProviderSource {
    name?: string
    is_connected?: boolean
    conversation_type?: ConversationType | string
  }

  interface Props {
    providers: Mark[]
    conversationType?: ConversationType | string
    status?: boolean
    size?: 'xs' | 'sm' | 'md'
    max?: number
    class?: string
  }

  let { providers, conversationType, status = false, size = 'sm', max = 4, class: className = '' }: Props = $props()

  const sizes = { xs: 'text-[11px]', sm: 'text-[12px]', md: 'text-[14px]' }

  const marks = $derived.by(() => {
    const seen = new Map<string, { icon: string; label: string; connected: boolean | undefined }>()
    for (const provider of providers) {
      const icon = providerIcon(provider, provider.conversation_type ?? conversationType)
      const label = provider.name ?? providerLabel(provider)
      const known = seen.get(icon)
      if (known) {
        known.label = known.label.includes(label) ? known.label : `${known.label}, ${label}`
        known.connected = known.connected === false || provider.is_connected === false ? false : provider.is_connected
      } else {
        seen.set(icon, { icon, label, connected: provider.is_connected })
      }
    }
    return [...seen.values()].slice(0, max)
  })
</script>

{#if marks.length}
  <span class="inline-flex shrink-0 items-center gap-1 {className}">
    {#each marks as mark (mark.icon)}
      <span class="relative inline-flex items-center" title={mark.label} aria-label={mark.label}>
        <Icon icon={mark.icon} class="{sizes[size]} grayscale" />
        {#if status && mark.connected !== undefined}
          <span class="absolute -right-0.5 -bottom-0.5 size-1.5 rounded-full ring-1 ring-paper-2 {mark.connected ? 'bg-success-500' : 'bg-error-500'}"></span>
        {/if}
      </span>
    {/each}
  </span>
{/if}
