import { describe, expect, it } from 'vitest'
import { composerCommands, type ComposerCommand } from './composerCommands'

const command = (id: string, label: string, keywords: string[] = [], hidden = false): ComposerCommand => ({ id, label, icon: 'lucide:dot', keywords, run: () => {}, hidden })

describe('composer commands', () => {
  const builtins = [command('draft', 'Draft a reply with AI', ['ai']), command('table', 'Table', ['grid']), command('off', 'Hidden', [], true)]
  const actions = [command('action:1', 'Summarise thread')]

  it('lists built ins before actions and drops hidden ones', () => {
    expect(composerCommands(builtins, actions, '').map((c) => `${c.group}:${c.id}`)).toEqual(['builtin:draft', 'builtin:table', 'action:action:1'])
  })

  it('filters on the label and keyword prefixes', () => {
    expect(composerCommands(builtins, actions, 'gr').map((c) => c.id)).toEqual(['table'])
    expect(composerCommands(builtins, actions, 'SUM').map((c) => c.id)).toEqual(['action:1'])
    expect(composerCommands(builtins, actions, 'reply').map((c) => c.id)).toEqual(['draft'])
    expect(composerCommands(builtins, actions, 'zzz')).toEqual([])
  })
})
