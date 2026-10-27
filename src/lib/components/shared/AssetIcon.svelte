<script lang="ts">
  import type { Asset } from '$lib/tauri/types';
  import Icon from '@iconify/svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';

  interface Props {
    asset: Asset | null | undefined;
    size?: number;
    class?: string;
    fallbackIcon?: string;
  }

  let {
    asset,
    size = 24,
    class: className = '',
    fallbackIcon = 'lucide:image'
  }: Props = $props();

  let isVideo = $derived(
    asset?.mime_type?.startsWith('video/') ||
    asset?.mime_type === 'image/gif'
  );
  let isSvg = $derived(asset?.mime_type === 'image/svg+xml');
  let isImage = $derived(
    asset?.mime_type?.startsWith('image/') &&
    !isSvg
  );

  let image = $state<HTMLImageElement | undefined>(undefined);

  $effect(() => {
    if (!asset) {
      image = undefined;
      return;
    }

    const img = new Image();
    img.onload = () => {
      image = img;
    };

    img.onerror = () => {
      image = undefined;
    };

    if (asset.path) {
      img.src = convertFileSrc(asset.path);
    } else if (asset.url) {
      img.src = asset.url;
    } else {
      image = undefined;
    }
  });
</script>

{#if !asset || !image}
  <Icon icon={fallbackIcon} width={size} height={size} class="text-ink-3 {className}" />
{:else if isVideo}
  <video
    src={image.src}
    width={size}
    height={size}
    class={className}
    autoplay
    loop
    muted
    playsinline
  >
    <Icon icon={fallbackIcon} width={size} height={size} class={className} />
  </video>
{:else}
  <img
    src={image.src}
    alt=""
    width={size}
    height={size}
    class="{isImage ? 'object-cover' : ''} {className}"
  />
{/if}
