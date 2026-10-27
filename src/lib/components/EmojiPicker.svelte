<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri'
  import type { ContentItem, WorkspaceEmoji } from '$lib/tauri/types'
  import { loadRecent, saveRecent, pushRecent, contentSrc, readFileAsBase64, RECENT_EMOJI_KEY, reactionUses, loadReactionUses, quickReactions } from '$lib/media'
  import { canonicalEmoji, customSrc, getEmojiGroups, groupWorkspaceEmoji, loadWorkspaceEmoji, searchEmoji, toneModifier, withSkinTone, workspaceEmoji, type StandardEmoji } from '$lib/emoji'
  import { providerIcon } from '$lib/providerIcons'
  import { anchored, type Align, type Box, type Side } from '$lib/popover'
  import { Field, IconButton, Sheet } from '$lib/components/ui'
  import EmojiGlyph from '$lib/components/shared/EmojiGlyph.svelte'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  interface Props {
    onselect: (emoji: string) => void;
    oncontent?: (item: ContentItem) => void;
    onclose: () => void;
    at?: Box | (() => Box | null) | null;
    side?: Side;
    align?: Align;
    providerId?: string | null;
  }

  let { onselect, oncontent, onclose, at = null, side = 'below', align = 'end', providerId = null }: Props = $props()
  const frequent = $derived(quickReactions($reactionUses, [], 16))
  let custom = $state<ContentItem[]>([])
  let custom_failure = $state<string | null>(null)
  let custom_input_el = $state<HTMLInputElement>()

  let search_query = $state('')
  let active_category = $state('recent')
  let picker_el = $state<HTMLDivElement>()
  const SKIN_TONE_KEY = 'chatnip_skin_tone'
  let skin_tone = $state(loadSkinTone())
  let skin_tone_open = $state(false)

  const SKIN_TONES: { label: MessageKey }[] = [
    { label: 'common.default' },
    { label: 'app.emoji.toneLight' },
    { label: 'app.emoji.toneMediumLight' },
    { label: 'app.emoji.toneMedium' },
    { label: 'app.emoji.toneMediumDark' },
    { label: 'app.emoji.toneDark' },
  ]

  const GROUP_LABELS: Record<string, { label: MessageKey; icon: string }> = {
    'smileys-emotion': { label: 'app.emoji.smileys', icon: 'lucide:smile' },
    'people-body': { label: 'app.emoji.people', icon: 'lucide:hand' },
    'animals-nature': { label: 'app.emoji.animals', icon: 'lucide:paw-print' },
    'food-drink': { label: 'app.emoji.food', icon: 'lucide:utensils' },
    'travel-places': { label: 'app.emoji.travel', icon: 'lucide:plane' },
    'activities': { label: 'app.emoji.activities', icon: 'lucide:trophy' },
    'objects': { label: 'app.emoji.objects', icon: 'lucide:lightbulb' },
    'symbols': { label: 'app.emoji.symbols', icon: 'lucide:hash' },
    'flags': { label: 'app.emoji.flags', icon: 'lucide:flag' },
  }

  const STANDARD_GROUPS = getEmojiGroups()

  interface Category {
    category: string;
    title: string;
    icon: string;
    codes?: string[];
    standard?: StandardEmoji[];
    workspace?: WorkspaceEmoji[];
    content?: boolean;
  }

  function loadSkinTone(): number {
    try {
      const stored = localStorage.getItem(SKIN_TONE_KEY)
      const value = stored ? parseInt(stored) : 0
      return value >= 0 && value < 6 ? value : 0
    } catch {
      return 0
    }
  }

  function saveSkinTone(index: number) {
    try {
      localStorage.setItem(SKIN_TONE_KEY, String(index))
    } catch {}
  }

  let recent_emojis = $state([...new Set(loadRecent(RECENT_EMOJI_KEY).map(canonicalEmoji))])

  function handleSelect(code: string) {
    const canonical = canonicalEmoji(code)
    recent_emojis = pushRecent(recent_emojis, canonical)
    saveRecent(recent_emojis, RECENT_EMOJI_KEY)
    onselect(canonical)
  }

  function standardCode(emoji: StandardEmoji): string {
    return withSkinTone(emoji, skin_tone).code
  }

  function standardGlyph(emoji: StandardEmoji): string {
    return withSkinTone(emoji, skin_tone).unicode
  }

  async function loadCustom() {
    if (!oncontent) return
    try {
      custom = await api.getContent('emoji')
    } catch (error) {
      custom_failure = String(error)
    }
  }

  async function handleCustomUpload(e: Event) {
    const input = e.target as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    try {
      const saved = await api.postContent({ kind: 'emoji', name: file.name.replace(/\.[^.]+$/, ''), data: await readFileAsBase64(file), mime_type: file.type || 'image/png' })
      custom = [saved, ...custom]
    } catch (error) {
      custom_failure = String(error)
    }
  }

  onMount(() => {
    loadCustom()
    loadReactionUses().catch(() => {})
    loadWorkspaceEmoji().catch(() => {})
  })

  function handleSkinToneSelect(index: number) {
    skin_tone = index
    saveSkinTone(index)
    skin_tone_open = false
  }

  const workspaces = $derived(groupWorkspaceEmoji(providerId ? $workspaceEmoji.filter((e) => e.provider_id === providerId) : $workspaceEmoji))

  let all_categories = $derived.by(() => {
    const categories: Category[] = []
    if (frequent.length > 0) categories.push({ category: 'frequent', title: t('app.emoji.frequent'), icon: 'lucide:flame', codes: frequent })
    if (recent_emojis.length > 0) categories.push({ category: 'recent', title: t('app.emoji.recent'), icon: 'lucide:clock', codes: recent_emojis })
    for (const group of STANDARD_GROUPS) {
      const known = GROUP_LABELS[group.key]
      categories.push({ category: group.key, title: known ? t(known.label) : group.label, icon: known?.icon ?? 'lucide:circle', standard: group.emojis })
    }
    for (const workspace of workspaces) {
      categories.push({ category: `workspace-${workspace.provider_id}`, title: t('app.emoji.workspace', { name: workspace.provider_name }), icon: providerIcon(workspace.provider_type), workspace: workspace.emojis })
    }
    if (oncontent) categories.push({ category: 'custom', title: t('app.emoji.custom'), icon: 'lucide:image', content: true })
    return categories
  })

  let visible_custom = $derived(search_query.trim() ? custom.filter(item => item.name.toLowerCase().includes(search_query.toLowerCase()) || item.tags.some(tag => tag.includes(search_query.toLowerCase()))) : custom)

  let filtered_emojis = $derived.by(() => {
    $workspaceEmoji
    if (!search_query.trim()) return null
    return searchEmoji(search_query, { limit: 120, labels: true, providerId })
  })

  function scrollToCategory(category: string) {
    active_category = category
    const el = document.getElementById(`emoji-cat-${category}`)
    el?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }

  const cell = 'flex size-9 items-center justify-center rounded-[0.5rem] text-[18px] hover:bg-paper-3'
</script>

<div bind:this={picker_el} class="w-80" use:anchored={{ at, side, align, onclose }}>
  <Sheet label={t('app.emoji.title')} width="20rem" anchored onclose={onclose}>
    <div class="px-2 pt-2">
      <Field bind:value={search_query} size="sm" placeholder={t('app.emoji.search')} autofocus />
    </div>

    {#if !search_query.trim()}
      <div class="flex items-center gap-0.5 overflow-x-auto px-2 pt-1.5">
        {#if skin_tone_open}
          {#each SKIN_TONES as tone, i (tone)}
            <button onclick={() => handleSkinToneSelect(i)} class="flex size-7 items-center justify-center rounded-[0.5rem] text-[15px] hover:bg-paper-3 {skin_tone === i ? 'bg-accent-soft' : ''}" aria-label={t(tone.label)}>👋{toneModifier(i)}</button>
          {/each}
          <IconButton icon="lucide:x" label={t('app.emoji.closeTones')} size="sm" class="ml-auto" onclick={() => (skin_tone_open = false)} />
        {:else}
          {#each all_categories as cat (cat.category)}
            <IconButton icon={cat.icon} label={cat.title} size="sm" active={active_category === cat.category} onclick={() => scrollToCategory(cat.category)} />
          {/each}
          <button onclick={() => (skin_tone_open = true)} class="ml-auto flex size-7 shrink-0 items-center justify-center rounded-[0.5rem] text-[15px] hover:bg-paper-3" aria-label={t('app.emoji.skinTone')}>👋{toneModifier(skin_tone)}</button>
        {/if}
      </div>
    {/if}

    <div class="max-h-[18rem] overflow-y-auto px-1.5 py-1.5">
      {#if search_query.trim() && filtered_emojis}
        <div class="grid grid-cols-8 gap-0.5">
          {#each filtered_emojis as match (match.custom ? `${match.custom.provider_id}:${match.name}` : match.name)}
            <button onclick={() => handleSelect(`:${match.name}:`)} class={cell} title={`:${match.name}:`} data-emoji={`:${match.name}:`}>
              {#if match.custom}
                <img src={customSrc(match.custom)} alt={`:${match.name}:`} class="size-5 object-contain" />
              {:else}
                <EmojiGlyph emoji={`:${match.name}:`} {providerId} />
              {/if}
            </button>
          {/each}
        </div>
        {#if oncontent && visible_custom.length > 0}
          <p class="px-1 py-1 text-[13px] text-ink-3">{t('app.emoji.custom')}</p>
          <div class="grid grid-cols-8 gap-0.5">
            {#each visible_custom as item (item.id)}
              <button onclick={() => oncontent?.(item)} class="flex size-9 items-center justify-center rounded-[0.5rem] hover:bg-paper-3" title={item.name}>
                <img src={contentSrc(item)} alt={item.name} class="size-5 object-contain" />
              </button>
            {/each}
          </div>
        {/if}
        {#if filtered_emojis.length === 0 && visible_custom.length === 0}
          <p class="py-8 text-center text-[13px] text-ink-3">{t('app.emoji.none')}</p>
        {/if}
      {:else}
        {#each all_categories as cat (cat.category)}
          <div id="emoji-cat-{cat.category}" class="mb-2" data-category={cat.category}>
            <p class="sticky top-0 z-10 bg-[var(--glass)] px-1 py-1 text-[13px] text-ink-3">{cat.title}</p>
            {#if cat.content}
              <div class="grid grid-cols-8 gap-0.5">
                {#each custom as item (item.id)}
                  <button onclick={() => oncontent?.(item)} class="flex size-9 items-center justify-center rounded-[0.5rem] hover:bg-paper-3" title={item.name}>
                    <img src={contentSrc(item)} alt={item.name} class="size-5 object-contain" />
                  </button>
                {/each}
                <IconButton icon="lucide:plus" label={t('app.emoji.addCustom')} onclick={() => custom_input_el?.click()} class="size-9" />
              </div>
              {#if custom_failure}
                <p class="px-1 py-1 text-[12px] text-error-500">{custom_failure}</p>
              {/if}
              <input bind:this={custom_input_el} type="file" accept="image/*" class="hidden" onchange={handleCustomUpload} />
            {:else if cat.workspace}
              <div class="grid grid-cols-8 gap-0.5">
                {#each cat.workspace as emoji (emoji.name)}
                  <button onclick={() => handleSelect(`:${emoji.name}:`)} class={cell} title={`:${emoji.name}:`} data-emoji={`:${emoji.name}:`}>
                    {#if emoji.alias_for && !emoji.url && !emoji.file_path}
                      <EmojiGlyph emoji={`:${emoji.alias_for}:`} />
                    {:else}
                      <img src={customSrc(emoji)} alt={`:${emoji.name}:`} class="size-5 object-contain" />
                    {/if}
                  </button>
                {/each}
              </div>
            {:else if cat.standard}
              <div class="grid grid-cols-8 gap-0.5">
                {#each cat.standard as emoji (emoji.unicode)}
                  <button onclick={() => handleSelect(standardCode(emoji))} class={cell} title={`:${emoji.name}:`}>{standardGlyph(emoji)}</button>
                {/each}
              </div>
            {:else if cat.codes}
              <div class="grid grid-cols-8 gap-0.5">
                {#each cat.codes as code (code)}
                  <button onclick={() => handleSelect(code)} class={cell} title={code}><EmojiGlyph emoji={code} {providerId} /></button>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  </Sheet>
</div>
