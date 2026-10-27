<script lang="ts">
  import { untrack } from 'svelte'
  import { boxOf, type Box } from '$lib/popover'
  import { modalsClose } from '$lib/modals'
  import { api, profileStore, providersStore, putAccountUsed } from '$lib/tauri'
  import type { StatusModalData } from '$lib/types'
  import { updatePresence } from '$lib/tauri/presence'
  import EmojiPicker from '$lib/components/EmojiPicker.svelte'
  import EmojiGlyph from '$lib/components/shared/EmojiGlyph.svelte'
  import { canonicalEmoji } from '$lib/emoji'
  import { accountName } from '$lib/accounts'
  import { Button, MenuItem, Select, Sheet } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  let { initial_emoji = '', initial_text = '', provider_id }: StatusModalData = $props()

  let status_emoji = $state(untrack(() => initial_emoji))
  let status_text = $state(untrack(() => initial_text))
  let failure = $state<string | null>(null)
  const account = $derived(provider_id ? (($providersStore.data ?? []).find((provider) => provider.id === provider_id) ?? null) : null)
  let duration = $state('none')
  let emoji_at = $state<Box | null>(null)

  const DURATIONS = $derived([
    { value: 'none', label: t('modals.status.dontClear') },
    { value: '30m', label: t('modals.status.minutes', { count: 30 }) },
    { value: '1h', label: t('common.hours', { count: 1 }) },
    { value: '4h', label: t('common.hours', { count: 4 }) },
    { value: 'today', label: t('common.today') },
    { value: 'week', label: t('modals.status.thisWeek') }
  ])

  const PRESETS = $derived([
    { emoji: canonicalEmoji('📅'), text: t('modals.status.meeting') },
    { emoji: canonicalEmoji('🚗'), text: t('modals.status.commuting') },
    { emoji: canonicalEmoji('🤒'), text: t('modals.status.sick') },
    { emoji: canonicalEmoji('🌴'), text: t('modals.status.vacation') },
    { emoji: canonicalEmoji('🏠'), text: t('modals.status.remote') }
  ])

  function calculateExpiration(): string | null {
    if (duration === 'none') return null
    const now = new Date()
    switch (duration) {
      case '30m':
        return new Date(now.getTime() + 30 * 60 * 1000).toISOString()
      case '1h':
        return new Date(now.getTime() + 60 * 60 * 1000).toISOString()
      case '4h':
        return new Date(now.getTime() + 4 * 60 * 60 * 1000).toISOString()
      case 'today': {
        const end = new Date(now)
        end.setHours(23, 59, 59, 999)
        return end.toISOString()
      }
      case 'week': {
        const end = new Date(now)
        const daysUntilSunday = 7 - end.getDay()
        end.setDate(end.getDate() + daysUntilSunday)
        end.setHours(23, 59, 59, 999)
        return end.toISOString()
      }
      default:
        return null
    }
  }

  function selectPreset(preset: { emoji: string; text: string }) {
    status_emoji = preset.emoji
    status_text = preset.text
  }

  function handleEmojiSelect(emoji: string) {
    status_emoji = emoji
    emoji_at = null
  }

  async function apply(text: string, emoji: string, expiration: string | null) {
    const user = account?.account?.user_id ?? $profileStore?.data?.user_id
    if (!user) return
    try {
      await api.putUserStatus({ status: 'online', status_text: text, status_emoji: emoji, status_expiration: expiration, provider_id: provider_id ?? null })
      updatePresence(user, { status: 'online', status_text: text, status_emoji: emoji, status_expiration: expiration })
      putAccountUsed(provider_id)
      modalsClose()
    } catch (error) {
      failure = String(error)
    }
  }
</script>

<Sheet label={t('modals.status.setTitle')} width="26rem" onclose={modalsClose}>
  <div class="px-5 pb-3 pt-5">
    <h2 class="text-[15px] font-semibold tracking-tight text-ink">{t('modals.status.title')}</h2>
    {#if account}
      <p class="mt-0.5 overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] text-ink-3">{accountName(account)} · {account.name}</p>
    {/if}
    <div class="mt-4 flex items-center gap-2 border-b border-line pb-2">
      <div class="relative">
        <button type="button" class="flex size-8 items-center justify-center rounded-[0.5rem] text-[16px] hover:bg-paper-3" aria-label={t('modals.status.pickEmoji')} onclick={(e) => (emoji_at = emoji_at ? null : boxOf(e.currentTarget))}>
          <EmojiGlyph emoji={status_emoji || ':grinning:'} />
        </button>
        {#if emoji_at}
          <EmojiPicker at={emoji_at} align="start" onselect={handleEmojiSelect} onclose={() => (emoji_at = null)} />
        {/if}
      </div>
      <input bind:value={status_text} placeholder={t('modals.status.placeholder')} class="flex-1 bg-transparent text-[14px] text-ink outline-none placeholder:text-ink-3" />
    </div>
    <div class="mt-3 flex items-center justify-between gap-3">
      <span class="text-[13.5px] text-ink-3">{t('modals.status.clearAfter')}</span>
      <Select bind:value={duration} options={DURATIONS} size="sm" />
    </div>
  </div>
  <div class="px-2.5 pb-3" role="menu">
    <p class="menu-label">{t('modals.status.presets')}</p>
    {#each PRESETS as preset (preset.text)}
      {@const on = status_emoji === preset.emoji && status_text === preset.text}
      <MenuItem selected={on} onclick={() => selectPreset(preset)}><EmojiGlyph emoji={preset.emoji} /> {preset.text}</MenuItem>
    {/each}
  </div>
  {#if failure}
    <p class="px-5 pb-3 text-[13px] text-error-500">{failure}</p>
  {/if}
  {#snippet footer()}
    <Button onclick={() => apply('', '', null)}>{t('modals.status.clear')}</Button>
    <Button class="ml-auto" onclick={modalsClose}>{t('common.cancel')}</Button>
    <Button variant="primary" onclick={() => apply(status_text, status_emoji, calculateExpiration())}>{t('common.save')}</Button>
  {/snippet}
</Sheet>
