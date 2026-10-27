<script lang="ts">
  import { api } from '$lib/tauri'
  import type { Provider, RemoteAutoReply } from '$lib/tauri/types'
  import { Button, Field, Row, Switch } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    provider: Provider
  }

  let { provider }: Props = $props()

  let reply = $state<RemoteAutoReply>({ enabled: false, subject: null, body: '', starts_at: null, ends_at: null, external_only: false })
  let scheduled = $state(false)
  let starts = $state('')
  let ends = $state('')
  let loaded = $state(false)
  let busy = $state(false)
  let saved = $state(false)
  let failure = $state<string | null>(null)

  const local = (iso: string | null) => {
    if (!iso) return ''
    const at = new Date(iso)
    return new Date(at.getTime() - at.getTimezoneOffset() * 60000).toISOString().slice(0, 16)
  }

  $effect(() => {
    const id = provider.id
    let live = true
    loaded = false
    api
      .getAutoReply(id)
      .then((found) => {
        if (!live || !found) return
        reply = found
        scheduled = !!(found.starts_at || found.ends_at)
        starts = local(found.starts_at)
        ends = local(found.ends_at)
      })
      .catch((error) => live && (failure = String(error)))
      .finally(() => live && (loaded = true))
    return () => {
      live = false
    }
  })

  async function save() {
    busy = true
    saved = false
    try {
      await api.putAutoReply(provider.id, {
        ...reply,
        subject: reply.subject?.trim() || null,
        starts_at: scheduled && starts ? new Date(starts).toISOString() : null,
        ends_at: scheduled && ends ? new Date(ends).toISOString() : null
      })
      saved = true
      failure = null
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }
</script>

<div data-testid="auto-reply" data-provider={provider.id} data-ready={loaded} class="border-b border-line pb-3 last:border-b-0">
  <Row label={provider.namespace ? `${provider.name} · ${provider.namespace}` : provider.name} hint={t('settings.autoReply.hint')}>
    <Switch testid="auto-reply-enabled" label={t('settings.autoReply.enabled')} bind:checked={reply.enabled} />
  </Row>
  {#if reply.enabled}
    <Row label={t('settings.autoReply.schedule')} hint={t('settings.autoReply.scheduleHint')}>
      <Switch testid="auto-reply-scheduled" label={t('settings.autoReply.schedule')} bind:checked={scheduled} />
      {#if scheduled}
        <Field size="sm" type="datetime-local" data-testid="auto-reply-starts" aria-label={t('settings.autoReply.starts')} bind:value={starts} />
        <Field size="sm" type="datetime-local" data-testid="auto-reply-ends" aria-label={t('settings.autoReply.ends')} bind:value={ends} />
      {/if}
    </Row>
    <Row label={t('settings.autoReply.subject')} for="auto-reply-subject-{provider.id}">
      <Field id="auto-reply-subject-{provider.id}" size="sm" data-testid="auto-reply-subject" class="w-[clamp(12rem,40%,22rem)]" bind:value={() => reply.subject ?? '', (value) => (reply.subject = value)} />
    </Row>
    <Row label={t('settings.autoReply.externalOnly')} hint={t('settings.autoReply.externalOnlyHint')}>
      <Switch testid="auto-reply-external" label={t('settings.autoReply.externalOnly')} bind:checked={reply.external_only} />
    </Row>
    <textarea
      data-testid="auto-reply-body"
      bind:value={reply.body}
      aria-label={t('settings.autoReply.body')}
      placeholder={t('settings.autoReply.bodyPlaceholder')}
      class="mt-2 min-h-[7rem] w-full resize-y border-b border-line bg-paper-2 px-2.5 py-2 text-[13px] leading-5 text-ink outline-none placeholder:text-ink-3 focus:border-ink-3"
    ></textarea>
  {/if}
  <div class="mt-2 flex items-center justify-end gap-2">
    {#if saved}<span class="text-[12px] text-ink-3">{t('settings.autoReply.saved')}</span>{/if}
    {#if failure}<span data-testid="auto-reply-failure" class="text-[12px] text-error-600">{failure}</span>{/if}
    <Button size="sm" variant="secondary" testid="auto-reply-save" busy={busy} disabled={!loaded} onclick={save}>{t('common.save')}</Button>
  </div>
</div>
