<script lang="ts">
  import { goto } from '$app/navigation'
  import { api } from '$lib/tauri'
  import ProfileAvatar from './ProfileAvatar.svelte'
  import ProfileForm from './ProfileForm.svelte'
  import { Button } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  let profileImage: string | null = $state(null)
  let profileName: string = $state('')
  let failure = $state<string | null>(null)

  async function handleSubmit() {
    if (!profileName.trim()) return
    failure = null
    try {
      await api.postProfile({ name: profileName.trim() })
      if (profileImage) await api.postProfileAvatar({ avatar_data: profileImage })
      await goto('/setup/providers')
    } catch (error) {
      failure = String(error)
    }
  }
</script>

<div class="mx-auto flex h-full max-w-[28rem] flex-col justify-center px-10 py-8">
  <h1 class="text-[28px] font-semibold leading-tight tracking-tight text-ink">{t('setup.profile.title')}</h1>
  <p class="mt-2 text-[14px] text-ink-2">{t('setup.profile.intro')}</p>

  <form
    class="mt-8 flex flex-col gap-5"
    onsubmit={(e) => {
      e.preventDefault()
      handleSubmit()
    }}
  >
    <ProfileAvatar bind:profileImage />
    <ProfileForm bind:profileName />
    {#if failure}
      <p class="text-[13px] text-error-500">{failure}</p>
    {/if}
    <div class="flex items-center gap-3">
      <Button type="submit" variant="primary" keys="↵" disabled={!profileName.trim()}>{t('common.continue')}</Button>
      <span class="font-mono text-[11px] text-ink-3">{t('setup.step', { step: 2, total: 4 })}</span>
    </div>
  </form>
</div>
