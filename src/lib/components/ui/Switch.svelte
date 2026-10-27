<script lang="ts">
  interface Props {
    checked: boolean
    label: string
    disabled?: boolean
    testid?: string
    onchange?: (checked: boolean) => void
  }

  let { checked = $bindable(), label, disabled = false, testid, onchange }: Props = $props()
</script>

<button
  type="button"
  role="switch"
  data-testid={testid}
  aria-checked={checked}
  aria-label={label}
  {disabled}
  class="relative inline-flex h-5 w-8 shrink-0 items-center rounded-full disabled:opacity-40 {checked ? 'bg-accent' : 'bg-line-strong'}"
  onclick={() => {
    checked = !checked
    onchange?.(checked)
  }}
>
  <span class="absolute top-0.5 size-4 rounded-full bg-paper shadow-[0_1px_2px_rgb(0_0_0/0.25)] {checked ? 'left-[0.875rem]' : 'left-0.5'}"></span>
</button>
