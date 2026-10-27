<div class="mx-auto flex h-full w-full max-w-[46rem] flex-col gap-4 px-5 pt-10">
  {#each Array(4) as _, i (i)}
    <div class="flex gap-3">
      <div class="size-8 shrink-0 animate-pulse rounded-full bg-paper-2"></div>
      <div class="flex-1 space-y-2">
        <div class="h-3 w-1/4 animate-pulse rounded bg-paper-2"></div>
        <div class="h-3 w-3/4 animate-pulse rounded bg-paper-2"></div>
        <div class="h-3 w-1/2 animate-pulse rounded bg-paper-2"></div>
      </div>
    </div>
  {/each}
</div>
