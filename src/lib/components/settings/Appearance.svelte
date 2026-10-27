<script lang="ts">
  import { recentColours, rememberColour } from '$lib/tauri/tags'
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri'
  import type { ContentItem, Person } from '$lib/tauri/types'
  import { contentSrc, readFileAsBase64 } from '$lib/media'
  import { Button, Segmented, Select, Switch } from '$lib/components/ui'
  import { conversationsStore, peopleStore } from '$lib/tauri'
  import { personOf } from '$lib/tauri/people'
  import { themeChoice, setThemeChoice, resolvedTheme, type ThemeChoice } from '$lib/theme-mode'
  import {
    ACCENTS,
    ART,
    GLOBAL_KEY,
    GRADIENTS,
    PREFIX,
    memberKey,
    SWATCHES,
    appearanceSettings,
    appearanceVars,
    backgroundClass,
    backgroundCss,
    globalAppearance,
    mergeAppearance,
    parseAppearance,
    patchAppearance,
    resolveTheme,
    resolveTint,
    saveAppearance,
    styleString,
    type AvatarForm,
    type AvatarScale,
    type Background,
    type Entry,
    type Intensity,
    type Motion,
    type Presence,
    type ScopedAppearance,
    type ThemeOverride,
    type Surface,
    type Tint
  } from '$lib/appearance'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  interface Props {
    scope?: string
  }

  let { scope }: Props = $props()

  const key = $derived(scope ?? GLOBAL_KEY)
  const conversationId = $derived(scope?.startsWith(PREFIX + 'conversation.') ? scope.slice((PREFIX + 'conversation.').length) : null)
  const members = $derived(conversationId ? ($conversationsStore.data.find((c) => c.id === conversationId)?.members ?? []).map((user) => personOf($peopleStore, { user_id: user.id })).filter((person): person is Person => !!person) : [])
  let member = $state('')
  const target = $derived(member && conversationId ? memberKey(member, conversationId) : key)
  const targetLayer = $derived(parseAppearance($appearanceSettings[target]))
  const layer = $derived(parseAppearance($appearanceSettings[key]))
  const effective = $derived(scope ? mergeAppearance($globalAppearance, layer) : $globalAppearance)
  const theme = $derived(resolveTheme(scope ? (layer.theme ?? 'inherit') : 'inherit', resolvedTheme($themeChoice)))
  const tint = $derived(resolveTint(effective.tint, theme))
  const previewStyle = $derived(styleString(appearanceVars(effective, theme)))
  const previewBackdrop = $derived(backgroundCss(effective.background))
  const selected = $derived(effective.background)
  const customColour = $derived(selected.kind === 'colour' && !SWATCHES.includes(selected.value ?? ''))
  const customGradient = $derived(selected.kind === 'gradient' && !selected.value ? { from: selected.from ?? '#f6c38b', to: selected.to ?? '#6b4a8c' } : { from: '#f6c38b', to: '#6b4a8c' })

  let images = $state<ContentItem[]>([])
  let failure = $state<string | null>(null)
  let busy = $state(false)
  let fileInput = $state<HTMLInputElement | null>(null)

  onMount(() => {
    api.getContent('image').then((items) => (images = items)).catch(() => {})
  })

  function patch(value: ScopedAppearance) {
    failure = null
    for (const colour of [value.background?.kind === 'colour' ? value.background.value : null, value.background?.from, value.background?.to, value.accent, value.bubbleMine, value.bubbleTheirs]) rememberColour(colour)
    patchAppearance(key, value).catch((error) => (failure = String(error)))
  }

  function patchTarget(value: ScopedAppearance) {
    failure = null
    patchAppearance(target, value).catch((error) => (failure = String(error)))
  }

  function same(a: Background, b: Background): boolean {
    return a.kind === b.kind && a.value === b.value && a.src === b.src && a.from === b.from && a.to === b.to
  }

  async function upload(e: Event) {
    const input = e.target as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    busy = true
    try {
      const saved = await api.postContent({ kind: 'image', name: file.name.replace(/\.[^.]+$/, ''), data: await readFileAsBase64(file), mime_type: file.type || 'image/png' })
      images = [saved, ...images]
      patch({ background: { kind: 'image', src: contentSrc(saved) } })
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  const themes = $derived([
    { value: 'system', label: t('common.system'), icon: 'lucide:sun-moon' },
    { value: 'day', label: t('settings.appearance.light'), icon: 'lucide:sun' },
    { value: 'night', label: t('settings.appearance.dark'), icon: 'lucide:moon' }
  ])
  const overrides = $derived([
    { value: 'inherit', label: t('settings.appearance.inherit'), icon: 'lucide:corner-down-right' },
    { value: 'light', label: t('settings.appearance.light'), icon: 'lucide:sun' },
    { value: 'dark', label: t('settings.appearance.dark'), icon: 'lucide:moon' }
  ])
  const tints = $derived([
    { value: 'auto', label: t('settings.appearance.auto'), icon: 'lucide:sparkles' },
    { value: 'light', label: t('settings.appearance.light'), icon: 'lucide:sun' },
    { value: 'dark', label: t('settings.appearance.dark'), icon: 'lucide:moon' }
  ])
  const intensities = $derived([
    { value: 'soft', label: t('settings.appearance.intensitySoft') },
    { value: 'normal', label: t('settings.appearance.intensityNormal') },
    { value: 'vivid', label: t('settings.appearance.intensityVivid') }
  ])
  const motions = $derived([
    { value: 'slow', label: t('settings.appearance.motionSlow') },
    { value: 'normal', label: t('settings.appearance.motionNormal') },
    { value: 'fast', label: t('settings.appearance.motionFast') }
  ])
  const surfaces = $derived([
    { value: 'solid', label: t('settings.appearance.solid'), icon: 'lucide:square' },
    { value: 'glass', label: t('settings.appearance.glass'), icon: 'lucide:layers' }
  ])
  const forms = $derived([
    { value: 'round', label: t('settings.appearance.avatarRound'), icon: 'lucide:circle' },
    { value: 'square', label: t('settings.appearance.avatarSquare'), icon: 'lucide:square' }
  ])
  const scales = $derived([
    { value: 'small', label: t('settings.appearance.avatarSmall') },
    { value: 'normal', label: t('settings.appearance.avatarNormal') },
    { value: 'large', label: t('settings.appearance.avatarLarge') }
  ])
  const presences = $derived([
    { value: 'dots', label: t('settings.appearance.presenceDots') },
    { value: 'wave', label: t('settings.appearance.presenceWave') },
    { value: 'pulse', label: t('settings.appearance.presencePulse') },
    { value: 'none', label: t('common.none') }
  ])
  const entries = $derived([
    { value: 'rise', label: t('settings.appearance.entryRise') },
    { value: 'fade', label: t('settings.appearance.entryFade') },
    { value: 'slide', label: t('settings.appearance.entrySlide') },
    { value: 'none', label: t('common.none') }
  ])
  const memberChoices = $derived([{ value: '', label: t('settings.appearance.wholeChat') }, ...members.map((person) => ({ value: person.id, label: person.display_name }))])
  const PRESET_NAMES: Record<string, MessageKey> = {
    'warm-dusk': 'settings.appearance.preset.warmDusk',
    aurora: 'settings.appearance.preset.aurora',
    'deep-sea': 'settings.appearance.preset.deepSea',
    sand: 'settings.appearance.preset.sand',
    plum: 'settings.appearance.preset.plum',
    graphite: 'settings.appearance.preset.graphite',
    forest: 'settings.appearance.preset.forest',
    rose: 'settings.appearance.preset.rose',
    'meadow-flow': 'settings.appearance.preset.meadowFlow',
    'mesh-flow': 'settings.appearance.preset.meshFlow',
    clear: 'settings.appearance.preset.clear',
    'mesh-dawn': 'settings.appearance.preset.meshDawn',
    'mesh-ocean': 'settings.appearance.preset.meshOcean',
    orbs: 'settings.appearance.preset.orbs',
    dune: 'settings.appearance.preset.dune',
    'night-grid': 'settings.appearance.preset.nightGrid',
    'conic-bloom': 'settings.appearance.preset.conicBloom',
    meadow: 'settings.appearance.preset.meadow',
    lime: 'settings.appearance.preset.lime',
    amber: 'settings.appearance.preset.amber',
    ember: 'settings.appearance.preset.ember',
    teal: 'settings.appearance.preset.teal',
    ocean: 'settings.appearance.preset.ocean'
  }

  function presetName(preset: { key: string; name: string }): string {
    const name = PRESET_NAMES[preset.key]
    return name ? t(name) : preset.name
  }
  const bubbleSwatches = [...ACCENTS.map((a) => a.value), ...SWATCHES].map((value) => ({ key: value, value, name: value }))
  const colourRows = $derived<{ field: 'accent' | 'bubbleMine' | 'bubbleTheirs'; label: MessageKey; testid?: string; round: boolean; swatches: { key: string; value: string; name: string }[] }[]>([
    { field: 'accent', label: 'settings.appearance.accent', testid: 'appearance-accent', round: true, swatches: ACCENTS.map((a) => ({ key: a.key, value: a.value, name: presetName(a) })) },
    { field: 'bubbleMine', label: 'settings.appearance.bubbleMine', round: false, swatches: bubbleSwatches },
    { field: 'bubbleTheirs', label: 'settings.appearance.bubbleTheirs', round: false, swatches: bubbleSwatches }
  ])
  const tile = 'relative aspect-square min-w-0 rounded-[0.5rem] shadow-[inset_0_0_0_1px_var(--line)] outline-none focus-visible:ring-2 focus-visible:ring-accent'
  const on = 'ring-2 ring-accent ring-offset-2 ring-offset-[var(--paper-2)]'
  const grid = 'grid grid-cols-[repeat(auto-fill,minmax(1.75rem,1fr))] gap-1.5'
</script>

<div class="flex flex-col">
  <div id={scope ? undefined : 'appearance.theme'} class="border-b border-line py-3">
    <p class="mb-2 text-[13px] text-ink-3">{t('settings.appearance.theme')}</p>
    {#if scope}
      <Segmented label={t('settings.appearance.theme')} size="sm" value={layer.theme ?? 'inherit'} options={overrides} onchange={(v) => patch({ theme: v as ThemeOverride })} />
    {:else}
      <Segmented label={t('settings.appearance.theme')} size="sm" value={$themeChoice} options={themes} onchange={(v) => setThemeChoice(v as ThemeChoice)} />
    {/if}
  </div>

  <div id={scope ? undefined : 'appearance.background'} class="border-b border-line py-3">
    <p class="mb-2 text-[13px] text-ink-3">{t('settings.appearance.background')}</p>
    <div class={grid}>
      <button type="button" data-testid="appearance-background" data-key="none" class="{tile} flex items-center justify-center bg-paper-3 text-[11px] text-ink-3 {selected.kind === 'none' ? on : ''}" title={t('common.none')} aria-label={t('settings.appearance.noBackground')} onclick={() => patch({ background: { kind: 'none' } })}>∅</button>
      {#each SWATCHES as swatch (swatch)}
        <button type="button" data-testid="appearance-background" data-key="colour:{swatch}" class="{tile} {same(selected, { kind: 'colour', value: swatch }) ? on : ''}" style:background={swatch} title={swatch} aria-label={t('settings.appearance.colourSwatch', { value: swatch })} onclick={() => patch({ background: { kind: 'colour', value: swatch } })}></button>
      {/each}
      {#each $recentColours.filter((colour) => !SWATCHES.includes(colour)).slice(0, 4) as colour (colour)}
        <button type="button" data-testid="appearance-background" data-key="recent:{colour}" class="{tile} {same(selected, { kind: 'colour', value: colour }) ? on : ''}" style:background={colour} title={colour} aria-label={t('settings.appearance.colourSwatch', { value: colour })} onclick={() => patch({ background: { kind: 'colour', value: colour } })}></button>
      {/each}
      <label class="{tile} flex cursor-pointer items-center justify-center overflow-hidden bg-paper-3 text-[13px] text-ink-3 {customColour ? on : ''}" title={t('settings.appearance.customColour')} style:background={customColour ? selected.value : undefined}>
        +
        <input type="color" class="absolute inset-0 cursor-pointer opacity-0" value={selected.kind === 'colour' ? selected.value : '#808080'} onchange={(e) => patch({ background: { kind: 'colour', value: e.currentTarget.value } })} />
      </label>
    </div>
    <div class="mt-1.5 {grid}">
      {#each GRADIENTS as preset (preset.key)}
        <button type="button" data-testid="appearance-background" data-key="gradient:{preset.key}" class="{tile} {same(selected, { kind: 'gradient', value: preset.key }) ? on : ''}" style:background={backgroundCss({ kind: 'gradient', value: preset.key })} title={presetName(preset)} aria-label={presetName(preset)} onclick={() => patch({ background: { kind: 'gradient', value: preset.key } })}></button>
      {/each}
      {#each ART as preset (preset.key)}
        <button type="button" data-testid="appearance-background" data-key="art:{preset.key}" class="{tile} {same(selected, { kind: 'art', value: preset.key }) ? on : ''}" style:background={backgroundCss({ kind: 'art', value: preset.key })} title={presetName(preset)} aria-label={presetName(preset)} onclick={() => patch({ background: { kind: 'art', value: preset.key } })}></button>
      {/each}
    </div>
    <div class="mt-2 flex items-center gap-2 text-[12.5px] text-ink-3">
      <span>{t('settings.appearance.customGradient')}</span>
      <input type="color" aria-label={t('settings.appearance.gradientStart')} class="size-6 cursor-pointer rounded-[0.375rem] bg-transparent" value={customGradient.from} onchange={(e) => patch({ background: { kind: 'gradient', from: e.currentTarget.value, to: customGradient.to } })} />
      <input type="color" aria-label={t('settings.appearance.gradientEnd')} class="size-6 cursor-pointer rounded-[0.375rem] bg-transparent" value={customGradient.to} onchange={(e) => patch({ background: { kind: 'gradient', from: customGradient.from, to: e.currentTarget.value } })} />
    </div>
    <div class="mt-2 grid grid-cols-[repeat(auto-fill,minmax(3.5rem,1fr))] gap-1.5">
      {#each images as image (image.id)}
        <button type="button" class="{tile} aspect-[4/3] {same(selected, { kind: 'image', src: contentSrc(image) }) ? on : ''}" style:background={backgroundCss({ kind: 'image', src: contentSrc(image) })} title={image.name} aria-label={image.name} onclick={() => patch({ background: { kind: 'image', src: contentSrc(image) } })}></button>
      {/each}
      <button type="button" class="{tile} flex aspect-[4/3] items-center justify-center bg-paper-3 text-[12px] text-ink-3 hover:text-ink" disabled={busy} onclick={() => fileInput?.click()}>{busy ? '…' : t('common.upload')}</button>
      <input bind:this={fileInput} type="file" accept="image/*" class="hidden" onchange={upload} />
    </div>
    {#if !scope && backgroundClass(selected)}
      <div class="mt-3 grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-2 text-[12.5px] text-ink-3">
        <span>{t('settings.appearance.intensity')}</span>
        <Segmented label={t('settings.appearance.intensity')} size="sm" value={effective.intensity} options={intensities} onchange={(v) => patch({ intensity: v as Intensity })} />
        <span>{t('settings.appearance.motion')}</span>
        <Segmented label={t('settings.appearance.motion')} size="sm" value={effective.motion} options={motions} onchange={(v) => patch({ motion: v as Motion })} />
      </div>
    {/if}
  </div>

  <div id={scope ? undefined : 'appearance.surface'} class="border-b border-line py-3">
    <p class="mb-2 text-[13px] text-ink-3">{t('settings.appearance.surface')}</p>
    <Segmented label={t('settings.appearance.surface')} size="sm" value={layer.surface ?? (scope ? '' : 'solid')} options={surfaces} onchange={(v) => patch({ surface: v as Surface })} />
    <p class="mt-1.5 text-[12px] text-ink-3">{scope ? t('settings.appearance.surfaceHintScoped') : t('settings.appearance.surfaceHint')}</p>
  </div>

  <div id={scope ? undefined : 'appearance.frost'} class="border-b border-line py-3">
    <p class="mb-2 text-[13px] text-ink-3">{t('settings.appearance.frost')}</p>
    <Segmented label={t('settings.appearance.frostTint')} size="sm" value={layer.tint ?? (scope ? '' : 'auto')} options={tints} onchange={(v) => patch({ tint: v as Tint })} />
    {#if !scope}
      <div class="mt-3 grid grid-cols-[auto_minmax(0,1fr)_3ch] items-center gap-x-3 gap-y-2 text-[12.5px] text-ink-3">
        <label for="appearance-blur">{t('settings.appearance.blur')}</label>
        <input id="appearance-blur" type="range" min="0" max="40" step="1" class="accent-[var(--accent)]" value={effective.blur} onchange={(e) => patch({ blur: Number(e.currentTarget.value) })} />
        <span class="text-right font-mono">{effective.blur}</span>
        <label for="appearance-opacity">{t('settings.appearance.opacity')}</label>
        <input id="appearance-opacity" type="range" min="15" max="95" step="1" class="accent-[var(--accent)]" value={effective.opacity} onchange={(e) => patch({ opacity: Number(e.currentTarget.value) })} />
        <span class="text-right font-mono">{effective.opacity}</span>
      </div>
    {/if}
  </div>

  {#if !scope}
    <div id="appearance.avatars" class="border-b border-line py-3">
      <p class="mb-2 text-[13px] text-ink-3">{t('settings.appearance.avatars')}</p>
      <div class="flex flex-col gap-3">
        <Segmented label={t('settings.appearance.avatarForm')} size="sm" value={effective.avatarForm} options={forms} onchange={(v) => patch({ avatarForm: v as AvatarForm })} />
        <Switch label={t('settings.appearance.avatarBorder')} checked={effective.avatarBorder} onchange={(on) => patch({ avatarBorder: on })} />
        <Segmented label={t('settings.appearance.avatarScale')} size="sm" value={effective.avatarScale} options={scales} onchange={(v) => patch({ avatarScale: v as AvatarScale })} />
      </div>
      <p class="mt-1.5 text-[12px] text-ink-3">{t('settings.appearance.avatarsHint')}</p>
    </div>
  {/if}

  <div id={scope ? undefined : 'appearance.presence'} class="border-b border-line py-3">
    <p class="mb-2 text-[13px] text-ink-3">{t('settings.appearance.motionFor')}</p>
    {#if members.length}
      <Select size="sm" value={member} options={memberChoices} onchange={(v) => (member = v)} class="mb-3" />
    {/if}
    <div class="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-2 text-[12.5px] text-ink-3">
      <span>{t('settings.appearance.presence')}</span>
      <Segmented label={t('settings.appearance.presence')} size="sm" value={targetLayer.presence ?? (scope ? '' : effective.presence)} options={presences} onchange={(v) => patchTarget({ presence: v as Presence })} />
      <span>{t('settings.appearance.entry')}</span>
      <Segmented label={t('settings.appearance.entry')} size="sm" value={targetLayer.entry ?? (scope ? '' : effective.entry)} options={entries} onchange={(v) => patchTarget({ entry: v as Entry })} />
    </div>
    <p class="mt-1.5 text-[12px] text-ink-3">{scope ? t('settings.appearance.motionForHintScoped') : t('settings.appearance.motionForHint')}</p>
  </div>

  {#each colourRows as row (row.field)}
    <div id={scope || row.field !== 'accent' ? undefined : 'appearance.accent'} class="border-b border-line py-3">
      <p class="mb-2 text-[13px] text-ink-3">{t(row.label)}</p>
      <div class={grid}>
        <button type="button" data-testid={row.testid} data-key="default" class="{tile} flex items-center justify-center bg-paper-3 text-[11px] text-ink-3 {!effective[row.field] ? on : ''}" title={t('common.default')} aria-label={t('settings.appearance.defaultFor', { name: t(row.label) })} onclick={() => patch({ [row.field]: null })}>∅</button>
        {#each row.swatches as swatch (swatch.key)}
          <button type="button" data-testid={row.testid} data-key={swatch.key} class="{tile} {row.round ? 'rounded-full' : ''} {effective[row.field] === swatch.value ? on : ''}" style:background={swatch.value} title={swatch.name} aria-label={t('settings.appearance.swatchFor', { name: t(row.label), value: swatch.name })} onclick={() => patch({ [row.field]: swatch.value })}></button>
        {/each}
        {#each $recentColours.filter((colour) => !row.swatches.some((swatch) => swatch.value.toLowerCase() === colour)).slice(0, 4) as colour (colour)}
          <button type="button" data-key="recent:{colour}" class="{tile} {row.round ? 'rounded-full' : ''} {effective[row.field]?.toLowerCase() === colour ? on : ''}" style:background={colour} title={colour} aria-label={t('settings.appearance.swatchFor', { name: t(row.label), value: colour })} onclick={() => patch({ [row.field]: colour })}></button>
        {/each}
        <label class="{tile} flex cursor-pointer items-center justify-center bg-paper-3 text-[13px] text-ink-3 {row.round ? 'rounded-full' : ''}" title={t('settings.appearance.customColour')}>
          +
          <input type="color" class="absolute inset-0 cursor-pointer opacity-0" value={effective[row.field] ?? '#1f9d55'} onchange={(e) => patch({ [row.field]: e.currentTarget.value })} />
        </label>
      </div>
    </div>
  {/each}

  {#if scope}
    <div class="py-3">
      <Button size="sm" testid="appearance-reset" disabled={!$appearanceSettings[key]} onclick={() => saveAppearance(key, null).catch((error) => (failure = String(error)))}>{t('settings.appearance.resetDefault')}</Button>
    </div>
  {:else}
    <div class="py-3">
      <p class="mb-2 text-[13px] text-ink-3">{t('settings.appearance.preview')}</p>
      <div class="relative isolate grid h-32 grid-cols-[1fr_1.6fr_3fr] gap-2 overflow-hidden rounded-[0.75rem] p-2 shadow-[inset_0_0_0_1px_var(--line)] {previewBackdrop ? '' : 'bg-paper-2'}" style={previewStyle}>
        {#if previewBackdrop}<div class="absolute inset-0 -z-10" style:background={previewBackdrop}></div>{/if}
        <div class="flex flex-col gap-1.5 p-2 {previewBackdrop ? `frost frost-edge frost-${tint}` : 'rounded-[0.5rem] bg-paper'}">
          <span class="h-1.5 w-3/4 rounded-full bg-ink/60"></span>
          <span class="h-1.5 w-1/2 rounded-full bg-ink-3/50"></span>
          <span class="h-1.5 w-2/3 rounded-full bg-ink-3/50"></span>
        </div>
        <div class="flex min-w-0 flex-col gap-2 p-2 rounded-[0.5rem] bg-paper">
          <div class="flex min-w-0 items-center gap-1.5"><span class="size-1.5 shrink-0 rounded-full bg-accent"></span><span class="text-[11px] font-semibold text-ink">Ada</span><span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[11px] text-ink-3">{t('settings.appearance.sample.lunch')}</span></div>
          <div class="flex min-w-0 items-center gap-1.5 pl-3"><span class="text-[11px] text-ink-2">Grace</span><span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[11px] text-ink-3">{t('settings.appearance.sample.shipped')}</span></div>
        </div>
        <div class="flex flex-col justify-end gap-1.5 p-2 rounded-[0.5rem] bg-paper">
          <span class="self-start rounded-[0.75rem] rounded-bl-[0.25rem] bg-[var(--bubble-theirs)] px-2.5 py-1 text-[11.5px] text-[var(--bubble-theirs-ink)] shadow-[inset_0_0_0_1px_var(--line)]">{t('settings.appearance.sample.question')}</span>
          <span class="self-end rounded-[0.75rem] rounded-br-[0.25rem] bg-[var(--bubble-mine)] px-2.5 py-1 text-[11.5px] text-[var(--bubble-mine-ink)]">{t('settings.appearance.sample.answer')}</span>
        </div>
      </div>
    </div>
  {/if}

  {#if failure}
    <p class="text-[12.5px] text-error-600">{failure}</p>
  {/if}
</div>
