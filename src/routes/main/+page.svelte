<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import {onMount} from 'svelte';
  import {goto} from '$app/navigation';
  import {profileStore} from "$lib/tauri";
  import { getStartupSettings, getStartupTarget, MAIN_ROUTE } from '$lib/startup';
  import { inboxView, openGroup, openInbox, openTag } from '$lib/inbox';

  let lastId = $derived($profileStore?.data?.last_id)
  onMount(async () => {
    const target = await getStartupSettings().then(getStartupTarget).catch(() => null);
    if (target && target.path !== MAIN_ROUTE) {
      if (target.inbox) openInbox(target.inbox);
      if (target.view !== 'inbox') inboxView.set(target.view);
      if (target.group) openGroup(target.group);
      if (target.tag) openTag(target.tag);
      goto(target.path, { replaceState: true });
    } else if (lastId) {
      goto(conversationHref(lastId));
    } else {
      goto('/main/conversation');
    }
  });
</script>
