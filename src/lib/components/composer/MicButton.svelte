<script lang="ts">
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { IconButton } from '$lib/components/ui'
  import { formatHotkey, hotkeyMatches, voice, type VoiceTarget } from '$lib/voice.svelte'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    target: VoiceTarget
    conversationId?: string | null
    size?: 'sm' | 'md'
    hotkey?: boolean
  }

  let { target, conversationId = null, size = 'md', hotkey = true }: Props = $props()

  onMount(() => {
    if (!voice.setup) voice.load().catch(() => {})
  })

  const keys = $derived(hotkey ? formatHotkey(voice.hotkey) || undefined : undefined)
  const label = $derived(
    !voice.ready
      ? t(voice.requirement === 'voice.enabled' ? 'voice.switchedOff' : 'voice.needsSetup')
      : voice.phase === 'failed' && voice.error
        ? voice.error
        : voice.phase === 'starting'
          ? t('voice.starting')
          : voice.phase === 'cleaning'
            ? t('voice.cleaning')
            : voice.recording
              ? t('voice.stop')
              : t('voice.start')
  )
  const ring = $derived(voice.recording ? Math.min(1, voice.level) : 0)

  function press() {
    if (!voice.ready) {
      if (voice.setupHref) goto(voice.setupHref)
      return
    }
    voice.toggle(target, conversationId)
  }

  function keydown(event: KeyboardEvent) {
    if (!hotkey || !voice.ready || event.defaultPrevented || !hotkeyMatches(event, voice.hotkey)) return
    event.preventDefault()
    voice.toggle(target, conversationId)
  }
</script>

<svelte:window onkeydown={keydown} />

<span class="relative inline-flex shrink-0" data-testid="mic-button" data-ready={voice.ready} data-requirement={voice.requirement ?? ''}>
  {#if voice.recording}
    <span
      class="pointer-events-none absolute inset-0 rounded-[0.5rem] bg-accent-soft transition-transform duration-100"
      style:transform="scale({1 + ring * 0.45})"
      style:opacity={0.35 + ring * 0.65}
      aria-hidden="true"
    ></span>
  {/if}
  <IconButton
    icon={voice.ready ? 'lucide:mic' : 'lucide:mic-off'}
    {label}
    {keys}
    {size}
    active={voice.recording}
    tone={voice.phase === 'failed' ? 'danger' : 'default'}
    busy={voice.phase === 'starting' || voice.phase === 'cleaning'}
    class="relative"
    onclick={press}
  />
</span>
