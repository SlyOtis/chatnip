export interface ComposerCommand {
  id: string
  label: string
  icon: string
  keywords: string[]
  run: () => void
  group?: 'builtin' | 'action'
  hidden?: boolean
}

export function composerCommands(builtins: ComposerCommand[], actions: ComposerCommand[], query: string): ComposerCommand[] {
  const q = query.trim().toLowerCase()
  const matches = (command: ComposerCommand) => !q || command.label.toLowerCase().includes(q) || command.keywords.some((k) => k.toLowerCase().startsWith(q))
  return [...builtins.filter((c) => !c.hidden).map((c) => ({ ...c, group: 'builtin' as const })), ...actions.map((c) => ({ ...c, group: 'action' as const }))].filter(matches)
}
