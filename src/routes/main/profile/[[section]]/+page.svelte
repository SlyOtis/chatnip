<script lang="ts">
  import Icon from '@iconify/svelte';
  import { tick } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { api, profileStore } from '$lib/tauri';
  import { putProfilePatch } from '$lib/tauri/profile';
  import { COMPOSER_CATEGORY, SHORTCUT_PREFIX, boundCombo, comboFromEvent, comboKeys, comboOf, composerKeys, conflictOf, getShortcutsByCategory, shortcutList, type Shortcut } from '$lib/shortcuts';
  import { listCollapsed, LIST_COLLAPSED_KEY } from '$lib/inbox';
  import { Button, IconButton, Kbd, PaneGrid, Row, Section, Segmented, Select, Switch } from '$lib/components/ui';
  import Connections from '$lib/components/settings/Connections.svelte';
  import Sorting from '$lib/components/settings/Sorting.svelte';
  import Assistant from '$lib/components/settings/Assistant.svelte';
  import AiSetup from '$lib/components/settings/AiSetup.svelte';
  import CalendarSettings from '$lib/components/settings/CalendarSettings.svelte';
  import Media from '$lib/components/settings/Media.svelte';
  import Appearance from '$lib/components/settings/Appearance.svelte';
  import Layouts from '$lib/components/settings/Layouts.svelte';
  import Security from '$lib/components/settings/Security.svelte';
  import VoiceSettings from '$lib/components/settings/VoiceSettings.svelte';
  import SidebarSettings from '$lib/components/settings/SidebarSettings.svelte';
  import StartupPage from '$lib/components/settings/StartupPage.svelte';
  import SettingFields from '$lib/components/settings/SettingFields.svelte';
  import ModelChoice from '$lib/components/settings/ModelChoice.svelte';
  import { layoutSettings, messageSetting, putLayoutSetting } from '$lib/layoutSettings';
  import { parseHash, putLastSection, resolveRow, searchSettings, sectionOf, SETTING_SECTIONS, settingHref, settingRedirect, settingValue, type SettingAnchor, type SettingField, type SettingSection } from '$lib/settings';
  import type { SettingRegistry } from '$lib/tauri/types';
  import { getLocaleChoice, putLocale, parseLocaleChoice, t, type MessageKey } from '$lib/i18n.svelte';
  import { pulse } from '$lib/window';

  const WAIT_TRIES = 20;

  const actionFields: SettingField[] = [
    { key: 'actions.max_sends_per_hour', anchor: 'actions.send-limit', label: 'settings.actions.sendLimit', hint: 'settings.actions.sendLimitHint' },
    { key: 'receipts.batch', anchor: 'actions.receipt-batch', label: 'settings.actions.receiptBatch', hint: 'settings.actions.receiptBatchHint' },
    { key: 'receipts.threshold', anchor: 'actions.receipt-threshold', label: 'settings.actions.receiptThreshold', hint: 'settings.actions.receiptThresholdHint' },
  ];
  const developerFields: SettingField[] = [
    { key: 'intelligence.timeout_seconds', anchor: 'developer.ai-timeout', label: 'settings.ai.timeout', hint: 'settings.ai.timeoutHint' },
    { key: 'summaries.concurrency', anchor: 'developer.summary-concurrency', label: 'settings.ai.summaryConcurrency', hint: 'settings.ai.summaryConcurrencyHint' },
    { key: 'window.card', anchor: 'developer.launch-card', label: 'settings.developer.launchCard', hint: 'settings.developer.launchCardHint' },
    { key: 'mock.home', anchor: 'developer.mock-home', label: 'settings.developer.mockHome', hint: 'settings.developer.mockHomeHint', wide: true },
  ];
  const mailFields: SettingField[] = [
    { key: 'mail.send_delay_seconds', anchor: 'messages.mail-delay', label: 'settings.messages.mailDelay', hint: 'settings.messages.mailDelayHint' },
    { key: 'mail.send_notice', anchor: 'messages.mail-notice', label: 'settings.messages.mailNotice', hint: 'settings.messages.mailNoticeHint', wide: true },
    { key: 'mail.draft_notice', anchor: 'messages.mail-draft-notice', label: 'settings.messages.mailDraftNotice', hint: 'settings.messages.mailDraftNoticeHint', wide: true },
  ];

  const profile = $derived($profileStore?.data ?? null);
  const settings = $derived(profile?.settings ?? null);

  let failure = $state<string | null>(null);
  let linkPreviews = $state('on');
  let query = $state('');
  let scroller = $state<HTMLDivElement | null>(null);
  let registry = $state<SettingRegistry | null>(null);
  let handled = '';

  const hits = $derived(searchSettings(query, t));
  const param = $derived(page.params.section);
  const current = $derived<SettingSection>(param && sectionOf(param) ? (param as SettingSection) : (parseHash(page.url.hash)?.section ?? 'general'));
  const heading = $derived(sectionOf(current) ?? SETTING_SECTIONS[0]);

  $effect(() => {
    if (current !== 'messages') return;
    api.getAppSettings().then((all) => (linkPreviews = all['links.previews'] || 'on')).catch(() => {});
    api.getSettingRegistry().then((loaded) => (registry = loaded)).catch((error) => (failure = String(error)));
  });

  function putRegistry(key: string, value: string) {
    if (registry) registry = { ...registry, values: { ...registry.values, [key]: value } };
    api.putAppSetting(key, value).catch((error) => (failure = String(error)));
  }

  $effect(() => {
    const { pathname, hash } = page.url;
    const key = pathname + hash;
    if (key === handled) return;
    handled = key;
    const redirect = settingRedirect(param, hash);
    if (redirect) {
      goto(redirect, { replaceState: true, keepFocus: true, noScroll: true });
      return;
    }
    putLastSection(current);
    tick().then(() => (parseHash(hash) ? focusAnchor(hash) : scroller?.scrollTo({ top: 0 })));
  });

  const intervals = $derived([
    { value: '1', label: t('common.minutes', { count: 1 }), mono: true },
    { value: '5', label: t('common.minutes', { count: 5 }), mono: true },
    { value: '10', label: t('common.minutes', { count: 10 }), mono: true },
    { value: '30', label: t('common.minutes', { count: 30 }), mono: true },
    { value: '60', label: t('common.hours', { count: 1 }), mono: true },
  ]);

  function reducedMotion(): boolean {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }

  function focusAnchor(hash: string, tries = 0) {
    const parsed = parseHash(hash);
    if (!parsed) return;
    if (!document.getElementById(parsed.anchor) && tries < WAIT_TRIES) {
      setTimeout(() => focusAnchor(hash, tries + 1), 100);
      return;
    }
    const found = resolveRow(parsed.anchor, (id) => document.getElementById(id));
    if (!found) return;
    if (found.id === parsed.section) {
      scroller?.scrollTo({ top: 0 });
      return;
    }
    found.row.scrollIntoView({ block: 'center', behavior: reducedMotion() ? 'auto' : 'smooth' });
    pulse(found.row);
  }

  function open(anchor: SettingAnchor) {
    const href = settingHref(anchor);
    if (href === page.url.pathname + page.url.hash) {
      focusAnchor(`#${anchor}`);
      return;
    }
    goto(href, { keepFocus: true, noScroll: true });
  }

  let recording = $state<Shortcut | null>(null)
  let conflict = $state<string | null>(null)

  function startRecording(shortcut: Shortcut) {
    conflict = null
    recording = recording === shortcut ? null : shortcut
  }

  $effect(() => {
    const target = recording
    if (!target) return
    const capture = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopImmediatePropagation()
      if (e.key === 'Escape') {
        recording = null
        return
      }
      const combo = comboFromEvent(e)
      if (!combo) return
      const clash = conflictOf(combo, target, $layoutSettings)
      if (clash) {
        conflict = t('settings.shortcuts.taken', { keys: comboKeys(combo).join(''), name: t(clash.description) })
        return
      }
      conflict = null
      recording = null
      putLayoutSetting(SHORTCUT_PREFIX + comboOf(target), combo === comboOf(target) ? null : combo).catch((error) => (conflict = String(error)))
    }
    window.addEventListener('keydown', capture, true)
    return () => window.removeEventListener('keydown', capture, true)
  })

  async function putSetting(patch: Partial<NonNullable<typeof settings>>) {
    try {
      await putProfilePatch(patch);
      failure = null;
    } catch (error) {
      failure = String(error);
    }
  }

  const languages = $derived([
    { value: 'system', label: t('settings.language.system') },
    { value: 'en', label: 'English' },
    { value: 'nb', label: 'Norsk bokmål' },
  ]);

  const shortcutGroups = $derived([...getShortcutsByCategory($shortcutList).entries()]);
  const composing = $derived(composerKeys(messageSetting($layoutSettings, 'messages.enter_sends')));
</script>

{#snippet editableRow(shortcut: Shortcut)}
  {@const own = comboOf(shortcut)}
  {@const combo = boundCombo(shortcut, $layoutSettings)}
  <div class="grid h-8 grid-cols-[6rem_minmax(0,1fr)_1.75rem] items-center gap-4 border-b border-line last:border-b-0" data-testid="shortcut-row" data-combo={own}>
    <dt>
      <button type="button" data-testid="shortcut-edit" class="flex h-7 min-w-0 items-center rounded-[0.375rem] px-1 hover:bg-paper-3 {recording === shortcut ? 'bg-accent-soft' : ''}" title={t('settings.shortcuts.change')} onclick={() => startRecording(shortcut)}>
        {#if recording === shortcut}<span class="text-[12px] text-accent">{t('settings.shortcuts.press')}</span>{:else}<Kbd keys={comboKeys(combo).join(' ')} />{/if}
      </button>
    </dt>
    <dd class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[13.5px] text-ink">{t(shortcut.description)}</dd>
    {#if combo !== own}
      <IconButton size="sm" testid="shortcut-reset" icon="lucide:rotate-ccw" label={t('settings.shortcuts.reset', { keys: comboKeys(own).join('') })} onclick={() => putLayoutSetting(SHORTCUT_PREFIX + own, null)} />
    {/if}
  </div>
{/snippet}

{#snippet shortcutRow(keys: string[], description: MessageKey)}
  <div class="grid h-8 grid-cols-[6rem_minmax(0,1fr)] items-center gap-4 border-b border-line last:border-b-0">
    <dt><Kbd keys={keys.join(' ')} /></dt>
    <dd class="truncate text-[13.5px] text-ink">{t(description)}</dd>
  </div>
{/snippet}

<PaneGrid name="settings" class="bg-paper" panes={[{ key: 'nav', size: '13rem', min: '10rem', max: '22rem', edge: 'end', class: 'bg-paper-2' }, { key: 'content' }]}>
  {#snippet pane(key)}
  {#if key === 'nav'}
  <nav class="flex min-h-0 flex-1 flex-col gap-px overflow-y-auto px-2 pt-12 pb-4" aria-label={t('common.settings')} style:--icon-cell="1.75rem">
    <p class="mb-2 px-2 text-[15px] font-semibold tracking-tight text-ink">{t('common.settings')}</p>
    <label class="mb-2 grid h-8 grid-cols-[var(--icon-cell)_minmax(0,1fr)] items-center border-b border-line text-ink-3 focus-within:border-ink-3">
      <Icon icon="lucide:search" class="justify-self-center text-[14px]" />
      <input data-testid="settings-search" class="h-full min-w-0 bg-transparent text-[13.5px] text-ink outline-none placeholder:text-ink-3" type="text" placeholder={t('settings.search')} aria-label={t('settings.search')} bind:value={query} onkeydown={(e) => { if (e.key === 'Enter' && hits[0]) open(hits[0].anchor); if (e.key === 'Escape') query = '' }} />
    </label>
    {#if query.trim()}
      {#each hits as hit (hit.anchor)}
        <button type="button" data-testid="settings-hit" data-anchor={hit.anchor} class="grid min-h-8 grid-cols-[var(--icon-cell)_minmax(0,1fr)] items-center rounded-[0.5rem] py-1 text-left text-ink-2 hover:bg-paper-3 hover:text-ink" onclick={() => open(hit.anchor)}>
          <Icon icon={hit.section.icon} class="justify-self-center text-[14px] text-ink-3" />
          <span class="flex min-w-0 flex-col">
            <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[13.5px]">{hit.label}</span>
            <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[11.5px] text-ink-3">{hit.sectionLabel}</span>
          </span>
        </button>
      {:else}
        <p data-testid="settings-search-empty" class="px-2 py-2 text-[13px] text-ink-3">{t('settings.searchEmpty')}</p>
      {/each}
    {:else}
      {#each SETTING_SECTIONS as section (section.id)}
        {@const active = current === section.id}
        <a
          data-testid="settings-nav"
          data-section={section.id}
          href={settingHref(section.id)}
          class="grid h-8 grid-cols-[var(--icon-cell)_minmax(0,1fr)] items-center rounded-[0.5rem] text-left text-[14px] {active ? 'bg-paper-3 font-medium text-ink' : 'text-ink-2 hover:bg-paper-3 hover:text-ink'}"
          aria-current={active ? 'location' : undefined}
        >
          <Icon icon={section.icon} class="justify-self-center text-[14px] {active ? 'text-accent' : 'text-ink-3'}" />
          <span class="overflow-hidden text-ellipsis whitespace-nowrap">{t(section.label)}</span>
        </a>
      {/each}
    {/if}
  </nav>
  {:else}
  <div class="min-h-0 flex-1 overflow-y-auto" bind:this={scroller}>
    <div class="h-10"></div>
    <div class="mx-auto w-full max-w-[clamp(40rem,92%,90rem)] px-[clamp(1rem,3%,2.5rem)] pb-[20vh]">
      {#if failure}
        <div data-testid="settings-failure" class="mb-4 flex items-center gap-3 border-b border-line pb-3 text-[13px] text-ink">
          <Icon icon="lucide:circle-alert" class="text-[15px] text-error-500" />
          <p class="flex-1">{failure}</p>
          <Button size="sm" onclick={() => (failure = null)}>{t('common.dismiss')}</Button>
        </div>
      {/if}

      <section id={current} class="pb-4" aria-labelledby="{current}-title">
        <h1 id="{current}-title" class="flex items-center gap-2 border-b border-line pb-3 text-[20px] font-semibold tracking-tight text-ink">
          <Icon icon={heading.icon} class="text-[18px] text-ink-3" />
          {t(heading.label)}
        </h1>
        {@render body(current)}
      </section>
    </div>
  </div>
  {/if}
  {/snippet}
</PaneGrid>

{#snippet body(id: SettingSection)}
  {#if id === 'general'}
    {#if profile}
      <Section id="general.profile" title={t('settings.profile.title')}>
        <Row id="general.language" label={t('settings.language.label')} hint={t('settings.language.hint')}>
          <Segmented label={t('settings.language.label')} size="sm" value={getLocaleChoice()} options={languages} onchange={(v) => putLocale(parseLocaleChoice(v)).catch((error) => (failure = String(error)))} />
        </Row>
        <StartupPage />
      </Section>
    {:else}
      <div class="py-8 text-center">
        <p class="text-[14px] font-medium text-ink">{t('settings.profile.none')}</p>
      </div>
    {/if}
    <Section id="general.panels" title={t('settings.panels.title')} description={t('settings.panels.description')}>
      <Row label={t('settings.panels.sidebar')} hint={t('settings.panels.sidebarHint')}>
        <Switch testid="panel-sidebar" label={t('settings.panels.sidebar')} checked={!settings?.navigation_collapsed} onchange={(on) => putSetting({ navigation_collapsed: !on })} />
      </Row>
      <Row label={t('settings.panels.list')} hint={t('settings.panels.listHint')}>
        <Switch testid="panel-list" label={t('settings.panels.list')} checked={!$listCollapsed} onchange={(on) => putLayoutSetting(LIST_COLLAPSED_KEY, on ? null : 'true')} />
      </Row>
      <Row label={t('settings.panels.details')} hint={t('settings.panels.detailsHint')}>
        <Switch testid="panel-details" label={t('settings.panels.details')} checked={!!settings?.details_panel_open} onchange={(on) => putSetting({ details_panel_open: on })} />
      </Row>
    </Section>
    <Section id="general.shortcuts" title={t('settings.shortcuts.title')} description={t('settings.shortcuts.description')}>
      {#if conflict}<p class="pb-2 text-[12.5px] text-error-600" data-testid="shortcut-conflict">{conflict}</p>{/if}
      <div class="grid grid-cols-[repeat(auto-fill,minmax(min(100%,22rem),1fr))] gap-x-10">
        {#each shortcutGroups as [category, shortcuts] (category)}
          <div class="py-3">
            <p class="mb-1 text-[13px] font-medium text-ink-2">{t(category)}</p>
            <dl>
              {#each shortcuts as shortcut (shortcut.description + shortcut.key)}
                {@render editableRow(shortcut)}
              {/each}
            </dl>
          </div>
        {/each}
        <div class="py-3">
          <p class="mb-1 text-[13px] font-medium text-ink-2">{t(COMPOSER_CATEGORY)}</p>
          <dl>
            {#each composing as entry (entry.description)}
              {@render shortcutRow(entry.keys, entry.description)}
            {/each}
          </dl>
        </div>
      </div>
    </Section>
  {:else if id === 'appearance'}
    <p class="max-w-[68ch] pt-4 pb-2 text-[13.5px] leading-5 text-ink-3">{t('settings.appearance.description')}</p>
    <Appearance />
  {:else if id === 'messages'}
    <Section title={t('settings.messages.display')}>
      <Row id="messages.time-format" label={t('settings.messages.timeFormat')} hint={t('settings.messages.timeFormatHint')}>
        <Segmented label={t('settings.messages.timeFormat')} value={settings?.time_format ?? '24h'} options={[{ value: '24h', label: '14:30', mono: true }, { value: '12h', label: '2:30 PM', mono: true }]} onchange={(v) => putSetting({ time_format: v })} />
      </Row>
      <Row id="messages.timestamp-interval" label={t('settings.messages.timestampInterval')} hint={t('settings.messages.timestampIntervalHint')}>
        <Select size="sm" class="w-[clamp(9rem,30%,13rem)]" testid="timestamp-interval" dialog={t('settings.messages.timestampInterval')} value={String(settings?.message_timestamp_interval ?? 5)} options={intervals} onchange={(v) => putSetting({ message_timestamp_interval: Number(v) })} />
      </Row>
      <Row id="messages.breaks" label={t('settings.messages.breaks')} hint={t('settings.messages.breaksHint')}>
        <Select size="sm" class="w-[clamp(9rem,30%,13rem)]" testid="section-gap" dialog={t('settings.messages.breaks')} value={messageSetting($layoutSettings, 'messages.section_gap')} options={[{ value: 'off', label: t('common.off') }, { value: '15', label: t('common.minutes', { count: 15 }) }, { value: '60', label: t('settings.messages.hoursShort', { count: 1 }) }, { value: '180', label: t('settings.messages.hoursShort', { count: 3 }) }, { value: 'day', label: t('settings.messages.day') }]} onchange={(v) => putLayoutSetting('messages.section_gap', v).catch(() => {})} />
      </Row>
      <Row id="messages.save-eyes" label={t('settings.messages.saveEyes')} hint={t('settings.messages.saveEyesHint')}>
        <Segmented label={t('settings.messages.saveEyes')} size="sm" value={messageSetting($layoutSettings, 'messages.save_eyes')} options={[{ value: 'off', label: t('common.off') }, { value: 'on', label: t('common.on') }]} onchange={(v) => putLayoutSetting('messages.save_eyes', v).catch(() => {})} />
      </Row>
    </Section>
    <Section title={t('settings.messages.chat')}>
      <Row id="messages.sending" label={t('settings.messages.sending')} hint={t('settings.messages.sendingHint')}>
        <Segmented label={t('settings.messages.sending')} size="sm" value={messageSetting($layoutSettings, 'messages.enter_sends')} options={[{ value: 'enter', label: t('settings.messages.sends', { key: '↵' }), mono: true }, { value: 'mod', label: t('settings.messages.sends', { key: '⌘↵' }), mono: true }]} onchange={(v) => putLayoutSetting('messages.enter_sends', v)} />
      </Row>
      <Row id="messages.collapse" label={t('settings.messages.collapse')} hint={t('settings.messages.collapseHint')}>
        <Segmented label={t('settings.messages.collapse')} size="sm" value={messageSetting($layoutSettings, 'messages.collapse_lines')} options={[{ value: '8', label: '8' }, { value: '16', label: '16' }, { value: '30', label: '30' }, { value: '0', label: t('common.never') }]} onchange={(v) => putLayoutSetting('messages.collapse_lines', v)} />
      </Row>
      <Row id="messages.emoji-size" label={t('settings.messages.emojiOnly')} hint={t('settings.messages.emojiOnlyHint')}>
        <Segmented label={t('settings.messages.emojiSize')} size="sm" value={messageSetting($layoutSettings, 'messages.emoji_size')} options={[{ value: 'small', label: t('common.small') }, { value: 'medium', label: t('common.medium') }, { value: 'large', label: t('common.large') }]} onchange={(v) => putLayoutSetting('messages.emoji_size', v)} />
      </Row>
      <Row id="messages.edited" label={t('settings.messages.edited')} hint={t('settings.messages.editedHint')}>
        <Segmented label={t('settings.messages.edited')} size="sm" value={messageSetting($layoutSettings, 'messages.show_edited')} options={[{ value: 'on', label: t('common.show') }, { value: 'off', label: t('common.hide') }]} onchange={(v) => putLayoutSetting('messages.show_edited', v)} />
      </Row>
      <Row id="messages.link-previews" label={t('settings.messages.linkPreviews')} hint={t('settings.messages.linkPreviewsHint')}>
        <Segmented label={t('settings.messages.linkPreviews')} size="sm" value={linkPreviews} options={[{ value: 'on', label: t('common.on') }, { value: 'known-only', label: t('settings.messages.knownSites') }, { value: 'off', label: t('common.off') }]} onchange={(v) => { linkPreviews = v; api.putAppSetting('links.previews', v).catch(() => {}) }} />
      </Row>
    </Section>
    <Section title={t('settings.messages.email')}>
      <Row id="messages.folded-mail" label={t('settings.messages.folded')} hint={t('settings.messages.foldedHint')}>
        <Segmented label={t('settings.messages.folded')} size="sm" value={messageSetting($layoutSettings, 'messages.mail_summaries')} options={[{ value: 'ai', label: t('settings.messages.aiSummary') }, { value: 'preview', label: t('settings.messages.firstLines') }, { value: 'off', label: t('settings.messages.senderOnly') }]} onchange={(v) => putLayoutSetting('messages.mail_summaries', v)} />
      </Row>
      <Row id="messages.model-threads" label={t('settings.models.feature.threads')} hint={t('settings.models.feature.threadsHint')}>
        <ModelChoice label={t('settings.models.feature.threads')} testid="threads" feature="threads" />
      </Row>
      <Row id="messages.default-reply" label={t('settings.messages.defaultReply')} hint={t('settings.messages.defaultReplyHint')}>
        <Segmented label={t('settings.messages.defaultReply')} size="sm" value={messageSetting($layoutSettings, 'messages.default_reply')} options={[{ value: 'reply', label: t('settings.messages.reply') }, { value: 'all', label: t('settings.messages.replyAll') }]} onchange={(v) => putLayoutSetting('messages.default_reply', v)} />
      </Row>
      <SettingFields fields={mailFields} />
      <Row id="messages.mail-delay-actions" label={t('settings.messages.mailDelayActions')} hint={t('settings.messages.mailDelayActionsHint')}>
        <Segmented label={t('settings.messages.mailDelayActions')} size="sm" value={settingValue(registry, 'mail.delay_actions')} options={[{ value: 'off', label: t('common.off') }, { value: 'on', label: t('common.on') }]} onchange={(v) => putRegistry('mail.delay_actions', v)} />
      </Row>
      <Row id="messages.mail-quit" label={t('settings.messages.mailQuit')} hint={t('settings.messages.mailQuitHint')}>
        <Segmented label={t('settings.messages.mailQuit')} size="sm" value={settingValue(registry, 'mail.quit_policy')} options={[{ value: 'draft', label: t('settings.messages.keepDraft') }, { value: 'send', label: t('settings.messages.sendNow') }]} onchange={(v) => putRegistry('mail.quit_policy', v)} />
      </Row>
    </Section>
  {:else if id === 'layouts'}
    <Layouts />
  {:else if id === 'accounts'}
    <Connections />
  {:else if id === 'organise'}
    <Sorting part="organise" />
    <SidebarSettings />
  {:else if id === 'ai'}
    <Assistant />
  {:else if id === 'voice'}
    <VoiceSettings />
  {:else if id === 'actions'}
    <Sorting part="actions" />
    <div class="pb-4">
      <Row id="actions.model-assistant" label={t('settings.models.feature.assistant')} hint={t('settings.models.feature.assistantHint')}>
        <ModelChoice label={t('settings.models.feature.assistant')} testid="assistant" feature="assistant" />
      </Row>
      <SettingFields fields={actionFields} />
    </div>
  {:else if id === 'content'}
    <Media />
  {:else if id === 'calendar'}
    <CalendarSettings />
  {:else if id === 'privacy'}
    <Security />
  {:else if id === 'developer'}
    <div class="pt-4"><AiSetup part="developer" /></div>
    <div class="pt-2 pb-4"><SettingFields fields={developerFields} /></div>
  {/if}
{/snippet}
