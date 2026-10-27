<script lang="ts">
  import Sheet from './Sheet.svelte'
  import Field from './Field.svelte'
  import Button from './Button.svelte'
  import IconPicker from './IconPicker.svelte'
  import ColourSwatch from './ColourSwatch.svelte'
  import TagMark from './TagMark.svelte'
  import { api } from '$lib/tauri'
  import { TAG_COLOURS, loadTags } from '$lib/tauri/tags'
  import type { Tag, TagKind } from '$lib/tauri/types'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    tag?: Tag | null
    kind?: TagKind
    name?: string
    onsaved?: (tag: Tag) => void
    onclose: () => void
  }

  let { tag = null, kind = 'tag', name: initialName = '', onsaved, onclose }: Props = $props()

  let name = $state(tag?.name ?? initialName)
  let icon = $state<string | null>(tag?.icon ?? null)
  let color = $state(tag?.color ?? TAG_COLOURS[0])
  let picking = $state(false)
  let saving = $state(false)
  let failure = $state<string | null>(null)

  async function save() {
    const trimmed = name.trim()
    if (!trimmed || saving) return
    saving = true
    try {
      const base = tag ?? (await api.postTag({ name: trimmed, color, kind }))
      const saved = await api.putTag({ tag_id: base.id, name: trimmed, color, icon: icon ?? '' })
      await loadTags()
      onsaved?.(saved)
      onclose()
    } catch (error) {
      failure = String(error)
    } finally {
      saving = false
    }
  }
</script>

{#snippet mark()}
  <TagMark tag={{ name, color, icon }} size="md" />
{/snippet}

<Sheet label={tag ? t('app.tagEditor.edit') : t('app.tagEditor.new')} width="26rem" {onclose}>
  <form
    class="flex flex-col gap-3 p-4"
    onsubmit={(e) => {
      e.preventDefault()
      save()
    }}
  >
    <div class="flex items-center gap-2">
      <Field bind:value={name} glyph={mark} onicon={() => (picking = true)} placeholder={t('app.tagEditor.name')} aria-label={t('common.name')} autocomplete="off" />
      <ColourSwatch value={color} onpick={(next) => next && (color = next)} />
    </div>
    {#if failure}
      <p class="text-[13px] text-error-600">{failure}</p>
    {/if}
    <div class="flex justify-end gap-2">
      <Button size="sm" onclick={onclose}>{t('common.cancel')}</Button>
      <Button size="sm" variant="primary" type="submit" keys="↵" disabled={!name.trim()} busy={saving}>{tag ? t('common.save') : t('common.create')}</Button>
    </div>
  </form>
</Sheet>

{#if picking}
  <IconPicker value={icon} fallback="lucide:circle-small" resetLabel={t('app.tagEditor.useDot')} onpick={(next) => (icon = next)} onclose={() => (picking = false)} />
{/if}
