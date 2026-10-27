<script lang="ts">
  import WindowControls from './WindowControls.svelte';
  import { onMount } from 'svelte';
  import {getCurrentWindow} from "@tauri-apps/api/window";
  import { api } from '$lib/tauri';
  import { watchWindowFrame } from '$lib/window';

  onMount(() => {
    const stop = watchWindowFrame(getCurrentWindow(), () => api.putWindowPosition().catch(() => {}));
    return () => {
      stop.then((unwatch) => unwatch()).catch(() => {});
    };
  });

  async function startResize(direction: string, e: MouseEvent) {
    e.preventDefault();
    try {
      await getCurrentWindow().startResizeDragging(direction as any);
    } catch {
    }
  }
</script>
<WindowControls />
<div class="window-resize-handles">
  <div class="absolute z-[9999] top-0 left-2 right-2 h-1 cursor-ns-resize" onmousedown={(e) => startResize('North', e)} role="presentation"></div>
  <div class="absolute z-[9999] bottom-0 left-2 right-2 h-1 cursor-ns-resize" onmousedown={(e) => startResize('South', e)} role="presentation"></div>
  <div class="absolute z-[9999] left-0 top-2 bottom-2 w-1 cursor-ew-resize" onmousedown={(e) => startResize('West', e)} role="presentation"></div>
  <div class="absolute z-[9999] right-0 top-2 bottom-2 w-1 cursor-ew-resize" onmousedown={(e) => startResize('East', e)} role="presentation"></div>
  <div class="absolute z-[9999] top-0 left-0 w-2 h-2 cursor-nw-resize" onmousedown={(e) => startResize('NorthWest', e)} role="presentation"></div>
  <div class="absolute z-[9999] top-0 right-0 w-2 h-2 cursor-ne-resize" onmousedown={(e) => startResize('NorthEast', e)} role="presentation"></div>
  <div class="absolute z-[9999] bottom-0 left-0 w-2 h-2 cursor-sw-resize" onmousedown={(e) => startResize('SouthWest', e)} role="presentation"></div>
  <div class="absolute z-[9999] bottom-0 right-0 w-2 h-2 cursor-se-resize" onmousedown={(e) => startResize('SouthEast', e)} role="presentation"></div>
</div>
