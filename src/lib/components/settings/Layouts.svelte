<script lang="ts">
  import { Segmented } from '$lib/components/ui'
  import TemplateEditor from '$lib/components/actions/TemplateEditor.svelte'
  import { layoutSettings, messageLayoutKey, putLayoutSetting } from '$lib/layoutSettings'
  import { compileTemplate, failedTemplates, forgetTemplates, type TemplateKind } from '$lib/template'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  const KINDS: { value: TemplateKind; label: MessageKey; icon: string }[] = [
    { value: 'chat', label: 'settings.layouts.chat', icon: 'lucide:message-circle' },
    { value: 'mail', label: 'settings.layouts.mail', icon: 'lucide:mail' },
    { value: 'bot', label: 'settings.layouts.bot', icon: 'lucide:bot' },
    { value: 'invite', label: 'settings.layouts.invite', icon: 'lucide:calendar-check' },
    { value: 'notification', label: 'settings.layouts.notification', icon: 'lucide:bell-ring' },
    { value: 'digest', label: 'settings.layouts.digest', icon: 'lucide:newspaper' },
    { value: 'system', label: 'settings.layouts.system', icon: 'lucide:info' },
    { value: 'media', label: 'settings.layouts.media', icon: 'lucide:image' }
  ]

  let kind = $state<TemplateKind>('chat')
  let failure = $state<string | null>(null)
  let timer: ReturnType<typeof setTimeout> | null = null

  const custom = $derived(new Set(KINDS.filter((k) => $layoutSettings[messageLayoutKey(k.value)]?.trim()).map((k) => k.value)))

  function save(source: string) {
    if (timer) clearTimeout(timer)
    const target = kind
    timer = setTimeout(async () => {
      failure = null
      if (source.trim() && !(await compileTemplate(source)).ok) return
      failedTemplates.delete(`layout:${target}`)
      putLayoutSetting(messageLayoutKey(target), source.trim() ? source : null).catch((error) => (failure = String(error)))
    }, 400)
  }

  $effect(() => () => {
    if (timer) clearTimeout(timer)
    forgetTemplates()
  })
</script>

<div class="flex flex-col gap-3 pt-4" id="layouts.editor">
  <p class="max-w-[68ch] text-[13.5px] leading-5 text-ink-3">{t('settings.layouts.description')}</p>
  <Segmented label={t('settings.layouts.kind')} size="sm" value={kind} options={KINDS.map((k) => ({ value: k.value, label: custom.has(k.value) ? `${t(k.label)} •` : t(k.label), icon: k.icon }))} onchange={(v) => (kind = v as TemplateKind)} />
  {#if failure}<p class="text-[12.5px] text-error-600">{failure}</p>{/if}
  <div class="h-[clamp(28rem,72vh,60rem)] overflow-hidden rounded-[0.75rem] border border-line" data-testid="layouts-editor">
    {#key kind}
      <TemplateEditor title={t(KINDS.find((k) => k.value === kind)?.label ?? 'settings.layouts.chat')} {kind} value={$layoutSettings[messageLayoutKey(kind)] ?? ''} onchange={save} />
    {/key}
  </div>
</div>
