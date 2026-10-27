<script lang="ts">
  import { presenceStore, requestPresence, type PresenceStatus } from '$lib/tauri/presence'

  interface Props {
    user_id: string
    size?: 'sm' | 'md' | 'lg' | 'xl'
  }

  let { user_id, size = 'md' }: Props = $props()

  let status = $derived<PresenceStatus>($presenceStore.get(user_id)?.status ?? 'offline')

  $effect(() => {
    requestPresence([user_id])
  })

  const sizeMap = {
    sm: 'size-2',
    md: 'size-2.5',
    lg: 'size-3',
    xl: 'size-3.5',
  }

  let dotSize = $derived(sizeMap[size])

  let dotClasses = $derived.by(() => {
    switch (status) {
      case 'online':
        return `${dotSize} rounded-full bg-success-500`
      case 'away':
        return `${dotSize} rounded-full bg-warn-500`
      case 'dnd':
        return `${dotSize} rounded-full bg-error-500`
      case 'offline':
      default:
        return `${dotSize} rounded-full bg-line-strong`
    }
  })
</script>

<div class="absolute -bottom-0.5 -right-0.5 {dotClasses} ring-2 ring-paper"></div>
