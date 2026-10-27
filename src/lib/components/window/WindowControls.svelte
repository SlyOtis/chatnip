<script lang="ts">
  import Icon from '@iconify/svelte';
  import {onMount} from "svelte";
  import {getCurrentWindow} from "@tauri-apps/api/window";
  import {getPlatform} from "$lib/tauri/api";
  import {profileStore} from "$lib/tauri";
  import { t } from "$lib/i18n.svelte";
  import { launchSettled } from "$lib/window";

  let isMaximized = $state<boolean | undefined>(undefined);
  let design = $state<string | undefined>($profileStore?.data?.settings?.window_controls_style);
  let focused = $state(true);
  let native = $state(true);
  async function minimize() {
    await getCurrentWindow().minimize()
  }
  async function toggleMaximize() {
    await getCurrentWindow().toggleMaximize()
    isMaximized = !isMaximized
  }
  async function closeWindow() {
    await getCurrentWindow().close();
  }

  onMount(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload }) => (focused = payload));
    getCurrentWindow().isMaximized().then((state) => {
      isMaximized = state;
    })

    getPlatform().then((platform) => {
      native = platform === 'macos'
    })
    if (design === undefined) {
      getPlatform().then((platform) => {
        switch (platform) {
          case 'macos':
            return 'macos'
          case 'windows':
            return 'windows'
          default:
            return 'linux'
        }
      }).then(platform => {
        design = platform
      })
    }
    return () => {
      unlisten.then((stop) => stop()).catch(() => {});
    };
  })
</script>
{#if $launchSettled && !native && isMaximized !== undefined && design !== undefined}
    <div
            class="absolute top-0 z-[1000] flex h-9 select-none items-center pointer-events-auto group {design} {focused ? '' : 'blurred'}"
            role="toolbar"
            aria-label={t('app.window.controls')}
            tabindex="0"
    >
        <button class="light close-btn" onclick={closeWindow} title={t('common.close')} aria-label={t('common.close')}>
            <Icon icon="lucide:x" />
        </button>
        <button class="light minimize-btn" onclick={minimize} title={t('app.window.minimize')} aria-label={t('app.window.minimize')}>
            <Icon icon="lucide:minus" />
        </button>
        <button class="light maximize-btn" onclick={toggleMaximize} title={isMaximized ? t('app.window.restore') : t('app.window.maximize')} aria-label={isMaximized ? t('app.window.restore') : t('app.window.maximize')}>
            <Icon icon={isMaximized ? 'lucide:minimize-2' : 'lucide:plus'} />
        </button>
    </div>
{/if}
<style>
  .macos {
    left: var(--rail-pad, 0.5rem);
    padding-left: 0.75rem;
    gap: 0.5rem;
  }
  .windows,
  .linux {
    right: 0;
    padding-right: 0.75rem;
    gap: 0.5rem;
  }
  .light {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 0.75rem;
    height: 0.75rem;
    border-radius: 9999px;
    color: rgb(0 0 0 / 0.55);
    font-size: 0.5rem;
  }
  .macos .light {
    box-shadow: inset 0 0 0 0.5px rgb(0 0 0 / 0.22);
  }
  .macos .close-btn {
    background-color: #ff5f57;
  }
  .macos .minimize-btn {
    background-color: #febc2e;
  }
  .macos .maximize-btn {
    background-color: #28c840;
  }
  .macos .light:active {
    filter: brightness(0.85);
  }
  .macos.blurred .light {
    background-color: rgb(127 127 127 / 0.35);
    box-shadow: inset 0 0 0 0.5px rgb(0 0 0 / 0.12);
  }
  .macos .light > :global(*) {
    opacity: 0;
  }
  .macos.group:hover .light > :global(*) {
    opacity: 1;
  }
  .macos.blurred.group:hover .close-btn {
    background-color: #ff5f57;
  }
  .macos.blurred.group:hover .minimize-btn {
    background-color: #febc2e;
  }
  .macos.blurred.group:hover .maximize-btn {
    background-color: #28c840;
  }
  .windows .light,
  .linux .light {
    width: 1.75rem;
    height: 1.75rem;
    border-radius: 0.375rem;
    color: var(--ink-2);
    font-size: 0.875rem;
    background-color: transparent;
  }
  .windows .light:hover,
  .linux .light:hover {
    background-color: var(--paper-3);
  }
  .windows .close-btn,
  .linux .close-btn {
    order: 3;
  }
</style>
