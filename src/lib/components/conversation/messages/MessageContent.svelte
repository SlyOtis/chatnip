<script lang="ts">
  import LinkPreview from './LinkPreview.svelte'
  import { dismiss, getPreviewMode, isHtml, loadDismissed, previewUrls } from '$lib/links'
  import { linkMentions, mentionsOf, parseMarkdown, sanitizeHtml } from '$lib/content'
  import { fromMarkdown, toHtml } from '$lib/components/composer/markdown'
  import { peopleStore } from '$lib/tauri/people'
  import { renderEmojiHtml, workspaceEmoji } from '$lib/emoji'
  import { openLink } from '$lib/messageLink'

  interface Props {
    content: string;
    content_type: string;
    previews?: boolean;
    metadata?: string | null;
    providerId?: string | null;
  }

  let { content, content_type, previews = true, metadata = null, providerId = null }: Props = $props()

  let dismissed = $state(loadDismissed())
  let previews_on = $state(false)

  $effect(() => {
    if (!previews) return
    let live = true
    getPreviewMode().then((mode) => {
      if (live) previews_on = mode !== 'off'
    })
    return () => {
      live = false
    }
  })

  let detected_urls = $derived(previews && previews_on ? previewUrls(content, content_type, dismissed) : [])
  let claimed = new Map<string, string>()

  function claimFor(url: string) {
    return (key: string) => {
      const owner = claimed.get(key)
      if (owner && owner !== url) return false
      claimed.set(key, url)
      return true
    }
  }

  let rendered_html = $derived.by(() => {
    if (!content) return ''

    const html = isHtml(content_type) ? sanitizeHtml(content) : content_type === 'markdown' ? toHtml(fromMarkdown(content)) : parseMarkdown(content)

    $workspaceEmoji
    return renderEmojiHtml(linkMentions(html, mentionsOf(metadata), $peopleStore), providerId)
  })

  function follow(event: MouseEvent) {
    const link = (event.target as Element | null)?.closest?.('a[href]')
    if (!link || link.hasAttribute('data-person')) return
    const href = link.getAttribute('href') ?? ''
    if (!/^https?:/i.test(href)) return
    event.preventDefault()
    openLink(href).catch(() => {})
  }
</script>

<div class="message-content" role="presentation" onclick={follow}>
  {@html rendered_html}
</div>

{#if detected_urls.length > 0}
  <div class="mt-2 flex flex-col gap-2">
    {#each detected_urls as url (url)}
      <LinkPreview {url} claim={claimFor(url)} ondismiss={() => (dismissed = dismiss(url))} />
    {/each}
  </div>
{/if}

<style>
  .message-content {
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .message-content :global(p) {
    margin: 0.25em 0;
  }

  .message-content :global(strong),
  .message-content :global(b) {
    font-weight: 600;
  }

  .message-content :global(em),
  .message-content :global(i) {
    font-style: italic;
  }

  .message-content :global(del) {
    text-decoration: line-through;
    color: var(--ink-3);
  }

  .message-content :global(.inline-code) {
    background: var(--paper-3);
    padding: 0.1em 0.35em;
    border-radius: 0.3em;
    font-size: 0.86em;
    font-family: var(--font-mono);
    color: var(--ink);
  }

  .message-content :global(.code-block-wrapper),
  .message-content :global(codeblock),
  .message-content :global(pre:not(.code-block)) {
    position: relative;
    display: block;
    margin: 0.5rem 0;
    border-radius: 8px;
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--line);
    overflow: hidden;
  }

  .message-content :global(.code-block-header) {
    position: absolute;
    top: 0.375rem;
    right: 0.375rem;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding-left: 0.25rem;
    border-radius: 0.375rem;
    background: var(--paper-2);
    font-size: 11px;
    line-height: 1;
  }

  .message-content :global(.code-lang),
  .message-content :global(codeblock[class]::before) {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-3);
  }

  .message-content :global(codeblock[class]::before) {
    content: attr(class);
    position: absolute;
    top: 0.5rem;
    right: 0.625rem;
    line-height: 1;
    text-transform: lowercase;
  }

  .message-content :global(.code-copy-btn) {
    padding: 0.25rem;
    border-radius: 0.25rem;
    color: var(--ink-3);
    cursor: pointer;
    background: none;
    border: none;
    display: flex;
    align-items: center;
    transition: color var(--fast);
  }

  .message-content :global(.code-copy-btn svg) {
    width: 11px;
    height: 11px;
  }

  .message-content :global(.code-copy-btn:hover) {
    color: var(--ink);
  }

  .message-content :global(.code-block),
  .message-content :global(codeblock),
  .message-content :global(pre:not(.code-block)) {
    padding: 0.625rem 0.75rem;
    margin: 0;
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 20px;
    white-space: pre;
    overflow-x: auto;
    overflow-wrap: normal;
    word-break: normal;
    color: var(--ink);
  }

  .message-content :global(.code-block-wrapper > .code-block) {
    padding-right: 4rem;
  }

  .message-content :global(codeblock) {
    margin: 0.5rem 0;
    padding-right: 4rem;
  }

  .message-content :global(.tok-keyword),
  .message-content :global(.tok-operatorKeyword) {
    color: var(--accent);
  }

  .message-content :global(.tok-string),
  .message-content :global(.tok-string2) {
    color: color-mix(in oklab, var(--accent) 55%, var(--ink));
  }

  .message-content :global(.tok-number),
  .message-content :global(.tok-bool),
  .message-content :global(.tok-atom) {
    color: color-mix(in oklab, var(--accent) 75%, var(--ink-2));
  }

  .message-content :global(.tok-comment),
  .message-content :global(.tok-meta) {
    color: var(--ink-3);
    font-style: italic;
  }

  .message-content :global(.tok-typeName),
  .message-content :global(.tok-className),
  .message-content :global(.tok-tagName),
  .message-content :global(.tok-propertyName),
  .message-content :global(.tok-attributeName) {
    color: var(--ink-2);
    font-weight: 600;
  }

  .message-content :global(.code-block code),
  .message-content :global(codeblock code),
  .message-content :global(pre code) {
    font-family: inherit;
    font-size: inherit;
    line-height: inherit;
    background: none;
    padding: 0;
    white-space: inherit;
  }

  .message-content :global(.msg-blockquote) {
    border-left: 2px solid var(--line-strong);
    padding-left: 0.75rem;
    color: var(--ink-2);
    margin: 0.25rem 0;
  }

  .message-content :global(.msg-link),
  .message-content :global(a:not([data-person])) {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 0.18em;
  }

  .message-content :global(.reply-quote) {
    display: block;
    margin: 0.125rem 0 0.375rem;
    padding: 0.25rem 0.625rem;
    border-left: 2px solid var(--line-strong);
    color: var(--ink-2);
    font-size: 0.9em;
  }

  .message-content :global(.reply-quote p) {
    margin: 0;
  }

  .message-content :global(.msg-heading) {
    margin: 0.5em 0 0.25em;
    font-weight: 600;
    line-height: 1.3;
    letter-spacing: -0.01em;
  }
  .message-content :global(.msg-heading-1) {
    font-size: 1.2em;
  }
  .message-content :global(.msg-heading-2) {
    font-size: 1.1em;
  }
  .message-content :global(.msg-heading-3),
  .message-content :global(.msg-heading-4),
  .message-content :global(.msg-heading-5),
  .message-content :global(.msg-heading-6) {
    font-size: 1em;
  }
  .message-content :global(.msg-rule) {
    margin: 0.5em 0;
    border: 0;
    border-top: 1px solid var(--line-strong);
  }
  .message-content :global(.msg-table) {
    margin: 0.35em 0;
    width: 100%;
    border-collapse: collapse;
    font-size: 0.95em;
  }
  .message-content :global(.msg-table th),
  .message-content :global(.msg-table td) {
    padding: 0.25em 0.6em;
    border: 1px solid var(--line);
    text-align: left;
    vertical-align: top;
  }
  .message-content :global(.msg-table th) {
    font-weight: 600;
    background: var(--paper-3);
    color: var(--ink-2);
  }
  .message-content :global(.msg-task) {
    list-style: none;
    margin-left: -1.1em;
  }
  .message-content :global(.msg-task .msg-check) {
    display: inline-block;
    width: 1.1em;
    color: var(--ink-3);
  }
  .message-content :global(.msg-task[data-done="true"]) {
    color: var(--ink-3);
  }
  .message-content :global(.msg-task[data-done="true"] .msg-check) {
    color: var(--accent);
  }
  .message-content :global(.msg-list) {
    list-style-type: disc;
    padding-left: 1.5rem;
    margin: 0.25rem 0;
  }

  .message-content :global(.msg-list-ordered) {
    list-style-type: decimal;
    padding-left: 1.5rem;
    margin: 0.25rem 0;
  }

  .message-content :global(.msg-list li),
  .message-content :global(.msg-list-ordered li) {
    margin: 0.125rem 0;
  }

  .message-content :global(li .msg-list),
  .message-content :global(li .msg-list-ordered) {
    margin: 0;
  }

  .message-content :global(li .msg-list) {
    list-style-type: circle;
  }

  .message-content :global(.msg-task) {
    list-style: none;
    margin-left: -1.25rem;
  }

  .message-content :global(.msg-task input) {
    margin: 0 0.375rem 0 0;
    vertical-align: -0.125em;
    accent-color: var(--accent);
  }

  .message-content :global(.msg-task:has(input:checked)) {
    color: var(--ink-3);
    text-decoration: line-through;
  }

  .message-content :global(h1),
  .message-content :global(h2),
  .message-content :global(h3),
  .message-content :global(h4),
  .message-content :global(h5),
  .message-content :global(h6) {
    margin: 0.5em 0 0.25em;
    font-weight: 650;
    letter-spacing: -0.01em;
    color: var(--ink);
  }

  .message-content :global(h1) {
    font-size: 1.375em;
  }

  .message-content :global(h2) {
    font-size: 1.2em;
  }

  .message-content :global(h3) {
    font-size: 1.08em;
  }

  .message-content :global(hr) {
    margin: 0.5rem 0;
    border: 0;
    border-top: 1px solid var(--line-strong);
  }

  .message-content :global(u) {
    text-underline-offset: 0.18em;
  }

  .message-content :global(img) {
    max-width: 100%;
    border-radius: var(--radius-row);
    margin: 0.25rem 0;
  }

  .message-content :global(table) {
    border-collapse: collapse;
    margin: 0.5rem 0;
    max-width: 100%;
  }

  .message-content :global(th),
  .message-content :global(td) {
    border: 1px solid var(--line);
    padding: 0.375rem 0.5rem;
    font-size: 0.9em;
    vertical-align: top;
  }

  .message-content :global(th) {
    background: var(--paper-2);
    font-weight: 600;
  }
</style>
