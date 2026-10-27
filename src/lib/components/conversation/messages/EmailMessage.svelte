<script lang="ts" module>
  import { writable } from 'svelte/store'
  import { getAppSettings, putAppSetting } from '$lib/tauri/api'

  const imageSenders = writable<Set<string>>(new Set())
  let requested = false

  function loadImageSenders() {
    if (requested) return
    requested = true
    getAppSettings()
      .then((settings) => imageSenders.set(new Set(Object.entries(settings).filter(([key, value]) => key.startsWith('mail.images.') && value === 'always').map(([key]) => key.slice('mail.images.'.length)))))
      .catch(() => (requested = false))
  }

  async function alwaysLoad(sender: string) {
    await putAppSetting(`mail.images.${sender}`, 'always')
    imageSenders.update((senders) => new Set(senders).add(sender))
  }
</script>

<script lang="ts">
  import type { Message } from '$lib/tauri';
  import type { Snippet } from 'svelte';
  import { getContext, onMount } from 'svelte';
  import { conversationStore } from '$lib/tauri';
  import { Button } from '$lib/components/ui';
  import { splitQuoted } from '$lib/content';
  import { isHtml } from '$lib/links';
  import { folds, isOpen, toggleFold } from '$lib/collapse';
  import MessageContent from './MessageContent.svelte';
  import AttachmentPreview from './AttachmentPreview.svelte';
  import MailFrame from './MailFrame.svelte';
  import { elementSize } from '$lib/window';
  import { formatDate, t } from '$lib/i18n.svelte';

  interface Props {
    message: Message;
    header?: Snippet;
  }

  let { message, header }: Props = $props();

  let detailsOpen = $state(false);
  let loadOnce = $state(false);
  let blocked = $state(0);
  let foldable = $state(false);
  let bodyHeight = $state(0);
  let limit = $state(600);
  let failure = $state<string | null>(null);

  const repeatedSubjects = getContext<Set<string> | undefined>('subject-repeats');

  const html = $derived(isHtml(message.content_type));
  const sender = $derived((message.sender_email ?? '').trim().toLowerCase());
  const remote = $derived(loadOnce || (!!sender && $imageSenders.has(sender)));
  const quoteOpen = $derived(isOpen($folds, message.id, 'quote', false));
  const full = $derived(isOpen($folds, message.id, 'full', false));
  const tall = $derived(bodyHeight > limit);
  const text = $derived(html ? null : splitQuoted(message.content, false));
  const hasFold = $derived(html ? foldable : !!(text?.quoted || text?.signature));
  const listed = $derived((message.attachments ?? []).filter((a) => !(a.is_inline && a.content_id && message.content?.includes(`cid:${a.content_id}`))));

  function bare(subject: string): string {
    return subject.replace(/^(\s*(re|fwd?|sv|vs)\s*:\s*)+/i, '').trim().toLowerCase();
  }

  const showSubject = $derived.by(() => {
    const subject = message.subject?.trim();
    if (!subject) return null;
    if (repeatedSubjects) return repeatedSubjects.has(message.id) ? null : subject;
    const name = $conversationStore.data?.name ?? '';
    return bare(subject) === bare(name) ? null : subject;
  });

  const to = $derived((message.recipients ?? []).map((r) => r.name || r.address));
  const cc = $derived((message.cc_recipients ?? []).map((r) => r.name || r.address));
  const fullTo = $derived((message.recipients ?? []).map((r) => (r.name ? `${r.name} <${r.address}>` : r.address)));
  const fullCc = $derived((message.cc_recipients ?? []).map((r) => (r.name ? `${r.name} <${r.address}>` : r.address)));
  const summary = $derived.by(() => {
    if (to.length === 0 && cc.length === 0) return '';
    const head = to[0] ?? cc[0];
    const rest = to.length + cc.length - 1;
    return rest > 0 ? t('conversation.email.toMany', { name: head, count: rest }) : t('conversation.email.toOne', { name: head });
  });

  async function always() {
    failure = null;
    try {
      await alwaysLoad(sender);
    } catch (error) {
      failure = String(error);
    }
  }

  onMount(() => {
    loadImageSenders();
    limit = parseFloat(getComputedStyle(document.documentElement).fontSize) * 37.5;
  });
</script>

<div class="min-w-0 [&+&]:mt-5">
  {#if showSubject}
    <h2 class="mb-1.5 max-w-[68ch] text-[15px] font-semibold leading-6 tracking-tight text-ink">{showSubject}</h2>
  {/if}
  {@render header?.()}
  {#if summary}
    <button class="-ml-1 rounded px-1 text-left text-[12px] leading-5 text-ink-3 hover:bg-paper-3 hover:text-ink-2" onclick={() => (detailsOpen = !detailsOpen)} aria-expanded={detailsOpen}>
      {summary}
    </button>
    {#if detailsOpen}
      <dl class="mt-1 grid max-w-[68ch] grid-cols-[2.5rem_minmax(0,1fr)] gap-x-2 text-[12px] leading-5">
        {#if fullTo.length}
          <dt class="text-ink-3">{t('conversation.email.to')}</dt>
          <dd class="selectable font-mono text-ink-2">{fullTo.join(', ')}</dd>
        {/if}
        {#if fullCc.length}
          <dt class="text-ink-3">{t('conversation.email.cc')}</dt>
          <dd class="selectable font-mono text-ink-2">{fullCc.join(', ')}</dd>
        {/if}
        <dt class="text-ink-3">{t('conversation.email.date')}</dt>
        <dd class="font-mono text-ink-2">{formatDate(message.timestamp, { dateStyle: 'medium', timeStyle: 'short' })}</dd>
      </dl>
    {/if}
  {/if}

  <div class="relative mt-2 overflow-hidden {tall && !full ? 'max-h-[37.5rem] [mask-image:linear-gradient(to_bottom,black_80%,transparent)]' : ''}">
    <div use:elementSize={(size) => (bodyHeight = size.height)}>
      {#if html}
        <MailFrame html={message.content || ''} providerId={message.provider_id} attachments={message.attachments ?? []} {remote} quote={quoteOpen} bind:blocked bind:foldable />
      {:else if text}
        <div class="prose-mail frost-light rounded-[0.75rem] bg-[#fbfaf7] p-[clamp(0.75rem,3%,1.25rem)] text-[14px] leading-6 shadow-[0_0_0_1px_rgb(0_0_0/0.06),0_1px_3px_rgb(0_0_0/0.12)] selectable">
          <MessageContent content={text.visible} content_type={message.content_type} providerId={message.provider_id} />
          {#if quoteOpen && text.signature}
            <div class="mt-2 whitespace-pre-line text-[13px] text-ink-3">{text.signature}</div>
          {/if}
          {#if quoteOpen && text.quoted}
            <div class="mt-2 border-l-2 border-line-strong pl-3 text-ink-2">
              <MessageContent content={text.quoted} content_type={message.content_type} providerId={message.provider_id} previews={false} />
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  {#if hasFold || tall || (blocked > 0 && !remote)}
    <div class="-ml-2.5 mt-1 flex flex-wrap items-center gap-1">
      {#if hasFold}
        <Button size="sm" class="font-mono tracking-widest text-ink-3" title={quoteOpen ? t('conversation.email.hideQuoted') : t('conversation.email.showQuoted')} onclick={() => toggleFold(message.id, 'quote', false)}>···</Button>
      {/if}
      {#if tall}
        <Button size="sm" icon={full ? 'lucide:chevron-up' : 'lucide:chevron-down'} onclick={() => toggleFold(message.id, 'full', false)}>{full ? t('common.showLess') : t('common.showMore')}</Button>
      {/if}
      {#if blocked > 0 && !remote}
        <Button size="sm" icon="lucide:image" title={t('conversation.email.blocked', { count: blocked })} onclick={() => (loadOnce = true)}>{t('conversation.email.loadImages')}</Button>
        {#if sender}
          <Button size="sm" class="text-ink-3" onclick={always}>{t('conversation.email.alwaysLoad', { sender })}</Button>
        {/if}
      {/if}
    </div>
    {#if failure}
      <p class="mt-1 text-[12px] text-error-500">{failure}</p>
    {/if}
  {/if}

  {#if listed.length > 0}
    <AttachmentPreview attachments={listed} framed messageId={message.id} source={message.attachments ?? []} />
  {/if}
</div>
