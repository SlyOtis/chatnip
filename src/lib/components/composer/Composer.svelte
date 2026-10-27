<script lang="ts">
  import { tick } from 'svelte'
  import { anchored, boxOf, type Box } from '$lib/popover'
  import { MenuItem } from '$lib/components/ui'
  import EmojiGlyph from '$lib/components/shared/EmojiGlyph.svelte'
  import MarkdownInput from './MarkdownInput.svelte'
  import { blockText, emptyDoc, type Block, type BlockType, type Doc } from './markdown'
  import { searchEmoji, workspaceEmoji, type EmojiMatch } from '$lib/emoji'
  import { composerCommands, type ComposerCommand } from '$lib/composerCommands'
  import { conversationsById, conversationsStore } from '$lib/tauri/conversations'
  import { usersStore } from '$lib/tauri/users'
  import { api, providersStore } from '$lib/tauri'
  import { accountOf } from '$lib/accounts'
  import type { RemoteUserGroup } from '$lib/tauri/types'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    doc?: Doc
    conversationId?: string | null
    providerId?: string | null
    mail?: boolean
    placeholder?: string
    maxLines?: number
    fill?: boolean
    submitOnEnter?: boolean
    builtins?: ComposerCommand[]
    actions?: ComposerCommand[]
    onsubmit?: (andDone: boolean) => void
    onfiles?: (files: File[]) => void
    onkeydown?: (e: KeyboardEvent) => void
  }

  let { doc = $bindable(emptyDoc()), conversationId = null, providerId = null, mail = false, placeholder = '', maxLines = 10, fill = false, submitOnEnter = true, builtins = [], actions = [], onsubmit, onfiles, onkeydown }: Props = $props()

  type Mentionable = { key: string; id: string; name: string; detail?: string; kind: 'member' | 'broadcast' | 'group' | 'user' }

  let editor = $state<ReturnType<typeof MarkdownInput>>()
  let box = $state<HTMLDivElement | null>(null)
  let mentionQuery = $state<string | null>(null)
  let mentionIndex = $state(0)
  let channelQuery = $state<string | null>(null)
  let channelIndex = $state(0)
  let shortcodeQuery = $state<string | null>(null)
  let shortcodeIndex = $state(0)
  let commandIndex = $state(0)
  let dismissedSlash = $state<string | null>(null)
  let commandsAt = $state<Box | null>(null)
  let groups = $state<RemoteUserGroup[]>([])

  const conversation = $derived(conversationId ? ($conversationsById.get(conversationId) ?? null) : null)
  const capabilities = $derived(providerId ? (accountOf($providersStore.data, providerId)?.capabilities ?? null) : null)

  $effect(() => {
    const id = providerId
    groups = []
    if (!id || !capabilities?.supports_user_groups) return
    let live = true
    api.getUserGroups(id).then((found) => live && (groups = found)).catch(() => {})
    return () => {
      live = false
    }
  })

  const mentionResults = $derived.by<Mentionable[]>(() => {
    if (mentionQuery === null) return []
    const q = mentionQuery.toLowerCase()
    const hit = (...values: (string | null | undefined)[]) => values.some((v) => (v ?? '').toLowerCase().includes(q))
    const members = conversation?.members ?? []
    const memberIds = new Set(members.map((m) => m.id))
    const out: Mentionable[] = members.filter((u) => hit(u.display_name, u.email)).map((u) => ({ key: 'member:' + u.id, id: u.id, name: u.display_name || u.email || t('common.unknown'), detail: u.email ?? undefined, kind: 'member' }))
    if (!mail && conversation && conversation.type !== 'direct_message') {
      for (const id of ['here', 'channel', 'everyone']) {
        if (id.startsWith(q)) out.push({ key: 'broadcast:' + id, id, name: id, detail: t(`conversation.composer.mention.${id}` as 'conversation.composer.mention.here'), kind: 'broadcast' })
      }
    }
    for (const group of groups.filter((g) => hit(g.handle, g.name))) out.push({ key: 'group:' + group.external_id, id: 'group:' + group.external_id, name: group.handle, detail: group.name, kind: 'group' })
    for (const u of $usersStore.data.filter((u) => !memberIds.has(u.id) && hit(u.display_name, u.email))) {
      out.push({ key: 'user:' + u.id, id: u.id, name: u.display_name || u.email || t('common.unknown'), detail: u.email ?? undefined, kind: 'user' })
      if (out.length >= 12) break
    }
    return out.slice(0, 12)
  })

  const channelResults = $derived.by(() => {
    if (channelQuery === null || mail) return []
    const q = channelQuery.toLowerCase()
    return ($conversationsStore.data ?? []).filter((c) => c.type === 'channel' && c.id !== conversationId && (!providerId || c.providers.some((p) => p.id === providerId)) && c.name.toLowerCase().includes(q)).slice(0, 8)
  })

  const shortcodeResults = $derived.by(() => {
    $workspaceEmoji
    return shortcodeQuery ? searchEmoji(shortcodeQuery, { limit: 8 }) : []
  })

  const slash = $derived.by(() => {
    if (doc.length !== 1 || doc[0].type !== 'paragraph') return null
    const text = blockText(doc[0])
    const m = /^\/([\w-]*)$/.exec(text)
    return m && text !== dismissedSlash ? m[1] : null
  })

  const commands = $derived(composerCommands(builtins, actions, commandsAt ? '' : (slash ?? '')))
  const commandsOpen = $derived((!!commandsAt || slash !== null) && commands.length > 0)
  const mentionOpen = $derived(mentionResults.length > 0)
  const channelOpen = $derived(channelResults.length > 0 && !mentionOpen)
  const shortcodeOpen = $derived(shortcodeQuery !== null && shortcodeResults.length > 0 && !mentionOpen && !channelOpen)

  $effect(() => {
    slash
    commandIndex = 0
  })

  $effect(() => {
    if (commandsOpen && commandIndex >= 0) tick().then(() => document.querySelector<HTMLElement>('[data-testid="composer-command"][aria-selected]')?.scrollIntoView({ block: 'nearest' }))
  })

  export function popupOpen(): boolean {
    return commandsOpen || mentionOpen || channelOpen || shortcodeOpen
  }

  export function openCommands(at: Box | null) {
    commandsAt = commandsAt ? null : at
    if (commandsAt) editor?.focus()
  }

  export function focus() {
    editor?.focus()
  }

  export function format(kind: 'bold' | 'italic' | 'underline' | 'strike' | 'code' | 'link') {
    editor?.format(kind)
  }

  export function block(kind: BlockType, attrs: Partial<Block> = {}) {
    editor?.block(kind, attrs)
  }

  export function insertText(text: string) {
    editor?.insertText(text)
  }

  export function insertMarkdown(text: string) {
    editor?.insertMarkdown(text)
  }

  export function replaceText(previous: string, next: string): boolean {
    return editor?.replaceText(previous, next) ?? false
  }

  export function clear() {
    editor?.clear()
  }

  export function isBlank(): boolean {
    return editor?.isBlank() ?? true
  }

  function closeCommands() {
    commandsAt = null
    dismissedSlash = slash === null ? null : '/' + slash
  }

  function runCommand(command: ComposerCommand) {
    if (slash !== null) editor?.clear()
    commandsAt = null
    command.run()
  }

  function handleMention(query: string | null) {
    if (query !== mentionQuery) mentionIndex = 0
    mentionQuery = query
  }

  function handleChannel(query: string | null) {
    if (query !== channelQuery) channelIndex = 0
    channelQuery = query
  }

  function handleShortcode(query: string | null) {
    if (query !== shortcodeQuery) shortcodeIndex = 0
    shortcodeQuery = query
  }

  function selectMention(item: Mentionable) {
    if (mentionQuery === null) return
    editor?.insertMention(mentionQuery.length + 1, item.id, item.name)
    mentionQuery = null
  }

  function selectChannel(id: string, name: string) {
    if (channelQuery === null) return
    editor?.insertChannel(channelQuery.length + 1, id, name)
    channelQuery = null
  }

  function selectShortcode(match: EmojiMatch) {
    if (shortcodeQuery === null) return
    editor?.insertShortcode(shortcodeQuery.length + 1, `:${match.name}:`)
    shortcodeQuery = null
  }

  function cycle(e: KeyboardEvent, index: number, size: number, choose: () => void, close: () => void): number {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      return (index + 1) % size
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault()
      return (index - 1 + size) % size
    }
    if (e.key === 'Enter' || e.key === 'Tab') {
      e.preventDefault()
      choose()
    } else if (e.key === 'Escape') {
      e.preventDefault()
      close()
    }
    return index
  }

  function handleKeydown(e: KeyboardEvent) {
    if (commandsOpen && !mentionOpen) {
      commandIndex = cycle(e, commandIndex, commands.length, () => runCommand(commands[commandIndex]), closeCommands)
      return
    }
    if (mentionOpen) {
      mentionIndex = cycle(e, mentionIndex, mentionResults.length, () => selectMention(mentionResults[mentionIndex]), () => (mentionQuery = null))
      return
    }
    if (channelOpen) {
      channelIndex = cycle(e, channelIndex, channelResults.length, () => selectChannel(channelResults[channelIndex].id, channelResults[channelIndex].name), () => (channelQuery = null))
      return
    }
    if (shortcodeOpen) {
      shortcodeIndex = cycle(e, shortcodeIndex, shortcodeResults.length, () => selectShortcode(shortcodeResults[shortcodeIndex]), () => (shortcodeQuery = null))
      return
    }
    onkeydown?.(e)
  }
</script>

<div bind:this={box} class="relative min-h-0 {fill ? 'flex-1' : ''}">
  {#if mentionOpen}
    <div class="menu w-72" use:anchored={{ at: () => boxOf(box), side: 'above', align: 'start', onclose: () => (mentionQuery = null) }} role="menu" data-testid="composer-mentions">
      {#each mentionResults as item, i (item.key)}
        {#if i === 0 || mentionResults[i - 1].kind !== item.kind}
          <p class="menu-label">{t(`conversation.composer.mentionGroup.${item.kind}` as 'conversation.composer.mentionGroup.member')}</p>
        {/if}
        <MenuItem selected={i === mentionIndex} testid="composer-mention-option" value={item.id} detail={item.detail} onclick={() => selectMention(item)}>{item.kind === 'broadcast' || item.kind === 'group' ? '@' + item.name : item.name}</MenuItem>
      {/each}
    </div>
  {/if}
  {#if channelOpen}
    <div class="menu w-64" use:anchored={{ at: () => boxOf(box), side: 'above', align: 'start', onclose: () => (channelQuery = null) }} role="menu" data-testid="composer-channels">
      {#each channelResults as channel, i (channel.id)}
        <MenuItem selected={i === channelIndex} icon="lucide:hash" testid="composer-channel-option" value={channel.id} onclick={() => selectChannel(channel.id, channel.name)}>{channel.name}</MenuItem>
      {/each}
    </div>
  {/if}
  {#if shortcodeOpen}
    <div class="menu w-64" use:anchored={{ at: () => boxOf(box), side: 'above', align: 'start', onclose: () => (shortcodeQuery = null) }} role="menu" data-testid="composer-shortcodes" aria-label={t('app.emoji.suggestions')}>
      {#each shortcodeResults as match, i (match.custom ? `${match.custom.provider_id}:${match.name}` : match.name)}
        <MenuItem selected={i === shortcodeIndex} testid="composer-shortcode-option" value={match.name} detail={match.custom?.provider_name} onclick={() => selectShortcode(match)}><span class="inline-flex items-center gap-2"><EmojiGlyph emoji={`:${match.name}:`} providerId={match.custom?.provider_id ?? null} /><span>:{match.name}:</span></span></MenuItem>
      {/each}
    </div>
  {/if}
  {#if commandsOpen}
    <div class="menu w-72" use:anchored={{ at: commandsAt ?? (() => boxOf(box)), side: 'above', align: 'start', onclose: closeCommands }} role="menu" data-testid="composer-commands" aria-label={t('conversation.composer.commands')}>
      {#each commands as command, i (command.id)}
        {#if i === 0 || commands[i - 1].group !== command.group}
          <p class="menu-label">{command.group === 'action' ? t('conversation.composer.commandsActions') : t('conversation.composer.commandsBuiltIn')}</p>
        {/if}
        <MenuItem icon={command.icon} testid="composer-command" value={command.id} selected={i === commandIndex} onclick={() => runCommand(command)}>{command.label}</MenuItem>
      {/each}
    </div>
  {/if}
  <MarkdownInput bind:this={editor} bind:doc {placeholder} {maxLines} {fill} {submitOnEnter} {onsubmit} onmention={handleMention} onchannel={handleChannel} onshortcode={handleShortcode} {onfiles} onkeydown={handleKeydown} />
</div>
