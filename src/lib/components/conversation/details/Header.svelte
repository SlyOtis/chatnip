<script lang="ts">
  import { putDetailsOpen } from '$lib/tauri/profile';
  import { IconButton } from '$lib/components/ui';
  import { t } from '$lib/i18n.svelte';

</script>

<header class="flex shrink-0 items-center px-5 pb-2 pt-4">
  <h3 class="text-[14px] font-semibold text-ink">{t('conversation.details.title')}</h3>
  <IconButton testid="details-close" icon="lucide:x" label={t('common.close')} keys="⌘⇧D" class="ml-auto" onclick={() => putDetailsOpen(false)} />
</header>
