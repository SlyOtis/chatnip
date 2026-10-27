import { describe, expect, it } from 'vitest'
import { actionDefaults, actionSelection, claudeConnectionInput, configFields, emptyConfig, featureKey, featureSelection, formatBytes, insertVariable, kindLabel, LOCAL_NODE, missingModel, missingVariables, modelChoice, modelChoices, modelPicker, ollamaConnectionInput, ollamaStep, originName, parseModelChoice, parseSchema, pluginAction, pluginCommand, pluginHome, pluginOptions, pullPercent, templateVariables, type ModelDefaults } from './intelligence'
import type { ActionGraph, CliSetup, IntelligenceConnection, OllamaStatus } from '$lib/tauri/types'
import { SETTING_SECTIONS } from '$lib/settings'
import registry from '../../src-tauri/src/models/app_setting.rs?raw'

describe('templateVariables', () => {
  it('finds dotted go template variables once', () => {
    const template = 'Sort {{ .conversation.name }} using {{ .messages }} and {{- .messages | len }} labels {{ .labels }}'
    expect(templateVariables(template)).toEqual(['conversation', 'messages', 'labels'])
  })
  it('ignores plain text and pipelines without a dot', () => {
    expect(templateVariables('{{ now }} and {{ "x" }}')).toEqual([])
  })
  it('reports variables the prompt does not provide', () => {
    expect(missingVariables('{{ .messages }} {{ .weather }}', ['messages', 'labels'])).toEqual(['weather'])
  })
})

describe('insertVariable', () => {
  it('inserts a token at the cursor and moves the cursor past it', () => {
    const result = insertVariable('ab', 'messages', 1)
    expect(result.template).toBe('a{{ .messages }}b')
    expect(result.cursor).toBe(1 + '{{ .messages }}'.length)
  })
})

describe('configFields', () => {
  it('lists the fields per kind', () => {
    expect(configFields('claude_cli').map((f) => f.key)).toEqual(['binary', 'model'])
    expect(configFields('replicate').map((f) => f.key)).toEqual(['api_token', 'model'])
    expect(configFields('openai_compatible').map((f) => f.key)).toEqual(['base_url', 'api_key', 'model'])
    expect(configFields('command').map((f) => f.key)).toEqual(['command', 'args', 'stdin', 'output', 'model_arg', 'models_command'])
  })
  it('builds an empty config with option defaults', () => {
    expect(emptyConfig('command')).toMatchObject({ command: '', stdin: 'prompt', output: 'json' })
    expect(kindLabel('command')).toBe('Local command')
  })
})

describe('originName', () => {
  const connections: IntelligenceConnection[] = [
    { id: 'a', kind: 'claude_cli', name: 'Claude Code', config: {}, is_default: true, available: true, detail: null, models: [], created_at: '' },
    { id: 'b', kind: 'replicate', name: 'Replicate', config: {}, is_default: false, available: true, detail: null, models: [], created_at: '' }
  ]
  it('names the chosen origin or the default', () => {
    expect(originName({ connection_id: 'b' }, connections)).toBe('Replicate')
    expect(originName({ connection_id: null }, connections)).toBe('Claude Code (default)')
    expect(originName({ connection_id: null }, [])).toBe('No provider')
  })
})

describe('parseSchema', () => {
  it('returns null for empty text and throws on invalid json', () => {
    expect(parseSchema('  ')).toBeNull()
    expect(parseSchema('{"type":"object"}')).toBe('{"type":"object"}')
    expect(() => parseSchema('{')).toThrow()
  })
})

describe('local AI setup', () => {
  const ollama = (over: Partial<OllamaStatus> = {}): OllamaStatus => ({ installed: true, binary: '/opt/homebrew/bin/ollama', running: true, version: '0.12.3', host: 'http://localhost:11434', models: [], download_url: 'https://ollama.com/download', ...over })

  it('formats sizes and pull progress', () => {
    expect(formatBytes(5225388164)).toBe('5.2 GB')
    expect(formatBytes(2019393189)).toBe('2.0 GB')
    expect(formatBytes(14_000_000_000)).toBe('14 GB')
    expect(formatBytes(0)).toBe('')
    expect(pullPercent({ completed: 40, total: 160 })).toBe(25)
    expect(pullPercent({ completed: null, total: 160 })).toBeNull()
    expect(pullPercent(null)).toBeNull()
  })

  it('walks Ollama from install to ready', () => {
    expect(ollamaStep(ollama({ installed: false, running: false }))).toBe('install')
    expect(ollamaStep(ollama({ running: false }))).toBe('start')
    expect(ollamaStep(ollama())).toBe('pull')
    expect(ollamaStep(ollama({ models: [{ name: 'qwen3:8b', size_bytes: 1, parameter_size: null, family: null }] }))).toBe('ready')
  })

  it('builds connections from what was detected', () => {
    expect(claudeConnectionInput({ installed: true, binary: '/opt/homebrew/bin/claude', version: '2.1.0', signed_in: true, account: 'me@example.org', plan: null, model: null, models: ['opus'] }, '')).toEqual({ kind: 'claude_cli', name: 'Claude Code', config: { model: '', binary: '/opt/homebrew/bin/claude' } })
    expect(ollamaConnectionInput(ollama({ models: [{ name: 'gemma3:4b', size_bytes: 1, parameter_size: null, family: null }] }), '')).toEqual({ kind: 'ollama', name: 'Ollama', config: { host: 'http://localhost:11434', model: 'gemma3:4b' } })
    expect(configFields('ollama').map((f) => f.key)).toEqual(['host', 'model'])
    expect(kindLabel('ollama')).toBe('Ollama')
  })

  it('treats an unset or uninstalled Ollama model as missing', () => {
    expect(missingModel('qwen3:8b', ['qwen3:8b'])).toBe(false)
    expect(missingModel('gemma3', ['gemma3:latest'])).toBe(false)
    expect(missingModel('qwen3:8b', ['gemma3:4b'])).toBe(true)
    expect(missingModel('', ['gemma3:4b'])).toBe(true)
    expect(missingModel(undefined, [])).toBe(true)
  })
})

describe('modelPicker', () => {
  const linked = (id: string, kind: IntelligenceConnection['kind'], model: string, models: string[], is_default = false): IntelligenceConnection => ({ id, kind, name: id, config: { model }, is_default, available: true, detail: null, models, created_at: '' })
  const plain: ModelDefaults = { models: [], local: false }
  const none = { connection: '', model: '' }
  const claude = linked('claude', 'claude_cli', 'big', ['big', 'small', 'tiny'], true)
  const local = linked('local', 'ollama', 'mid', ['mid', 'small'])

  it('shows the default connection and its model, dimmed as the default', () => {
    expect(modelPicker(none, [local, claude], plain)).toEqual({ connection: claude, value: '', label: 'Default (claude · big)', defaultLabel: 'Default (claude · big)', own: false, missing: false })
  })

  it('reads the model Claude Code is set to when the default provider has none of its own', () => {
    const bare = linked('cc', 'claude_cli', '', ['opus', 'sonnet'], true)
    expect(modelPicker(none, [bare], plain, 'opus').defaultLabel).toBe('Default (cc · Claude Code default · opus)')
    expect(modelPicker(none, [bare], plain).defaultLabel).toBe('Default (cc · Claude Code default)')
    expect(modelPicker(none, [{ ...bare, is_default: false }, { ...local, is_default: true }], plain, 'opus').defaultLabel).toBe('Default (local · mid)')
  })

  it('shows its own choice as provider and model', () => {
    const values = { [featureKey('threads', 'connection')]: 'local', [featureKey('threads', 'model')]: 'small' }
    expect(modelPicker(featureSelection('threads', values), [claude, local], plain)).toMatchObject({ connection: local, value: modelChoice('local', 'small'), label: 'local · small', own: true, missing: false })
    expect(modelPicker(actionSelection({ ai: { connection_id: 'local', model: 'small' } }), [claude, local], plain)).toMatchObject({ connection: local, value: modelChoice('local', 'small'), own: true })
    expect(actionSelection({ ai: { connection_id: null, model: null } })).toEqual(none)
  })

  it('marks a saved model that the provider no longer has', () => {
    expect(modelPicker({ connection: 'local', model: 'custom:1b' }, [claude, local], plain)).toMatchObject({ own: true, missing: true, label: 'local · custom:1b' })
    expect(modelPicker({ connection: 'gone', model: '' }, [claude, local], plain)).toMatchObject({ connection: claude, value: '', own: true, missing: true })
  })

  it('names the registry default for the connection kind', () => {
    expect(modelPicker(none, [claude], { models: [['claude_cli', 'tiny']], local: false })).toMatchObject({ defaultLabel: 'Default (claude · tiny)', own: false })
  })

  it('prefers a local connection and the smallest installed model for an action that confirms receipts', () => {
    const graph: ActionGraph = { nodes: [{ id: 'confirm', type: LOCAL_NODE, label: '', config: {}, body: '', x: 0, y: 0 }], edges: [] }
    expect(actionDefaults(graph)).toEqual({ models: [], local: true })
    expect(actionDefaults({ nodes: [], edges: [] })).toEqual(plain)
    expect(modelPicker(none, [claude, local], actionDefaults(graph))).toMatchObject({ connection: local, defaultLabel: 'Default (local · Smallest installed model)' })
  })

  it('has nothing to offer without connections', () => {
    expect(modelPicker(none, [], plain)).toEqual({ connection: undefined, value: '', label: 'Add a provider first', defaultLabel: 'Add a provider first', own: false, missing: false })
  })

  it('lists only the models each provider reports, grouped by provider', () => {
    const plain = linked('plain', 'claude_cli', '', ['opus', 'sonnet'])
    expect(modelChoices([plain, local]).map((c) => [c.value, c.label, c.group])).toEqual([
      ['plain|', 'plain · Claude Code default', 'plain'],
      ['plain|opus', 'plain · opus', 'plain'],
      ['plain|sonnet', 'plain · sonnet', 'plain'],
      ['local|', 'local · mid', 'local'],
      ['local|small', 'local · small', 'local']
    ])
    expect(modelChoices([plain], 'opus')[0].label).toBe('plain · Claude Code default · opus')
    expect(parseModelChoice('a|b:1|c')).toEqual({ connection: 'a', model: 'b:1|c' })
    expect(parseModelChoice(modelChoice('a', ''))).toEqual({ connection: 'a', model: '' })
  })

  it('gives every feature in the Rust registry a settings row next to its other settings, and none on the AI page', () => {
    const ids = [...registry.matchAll(/ModelFeature \{ id: "([a-z]+)"/g)].map((m) => m[1])
    expect(ids).toEqual(['threads', 'assistant', 'voice'])
    const rows = SETTING_SECTIONS.flatMap((s) => s.settings.map((r) => `${s.id}.${r.id}`))
    for (const id of ids) expect([id, rows.filter((r) => r.endsWith(`.model-${id}`)).length]).toEqual([id, 1])
    expect(rows.filter((r) => r.startsWith('ai.model'))).toEqual([])
  })
})

describe('the developer plugin row', () => {
  const defaults = Object.fromEntries([...registry.matchAll(/\((PLUGIN_HOME|PLUGIN_MOCK|MOCK_HOME), "([^"]*)"\)/g)].map((m) => [{ PLUGIN_HOME: 'plugin.home', PLUGIN_MOCK: 'plugin.mock', MOCK_HOME: 'mock.home' }[m[1]] as string, m[2]]))
  const chosen = (values: Record<string, string>) => ({ defaults, values })
  const cli = (patch: Partial<CliSetup>): CliSetup => ({ executable: '/opt/chatnip', link: '', linked: false, on_path: false, claude: '/bin/claude', plugin_dir: '/u/.chatnip/plugin', plugin_version: null, plugin_current: '1-a', plugin_installed: false, plugin_options: null, ...patch })

  it('defaults to the app data home without forcing mock mode', () => {
    expect(defaults).toEqual({ 'plugin.home': 'app', 'plugin.mock': 'false', 'mock.home': '~/.chatnip-mock' })
    expect(pluginHome(chosen({}))).toBe('app')
    expect(pluginOptions(chosen({}))).toEqual({ home: null, mock: false })
    expect(pluginOptions(null)).toEqual({ home: null, mock: false })
  })
  it('uses the mock home setting and the forced mock switch', () => {
    expect(pluginOptions(chosen({ 'plugin.home': 'mock', 'plugin.mock': 'true' }))).toEqual({ home: '~/.chatnip-mock', mock: true })
    expect(pluginOptions(chosen({ 'plugin.home': 'mock', 'mock.home': '/tmp/cn-mock' }))).toEqual({ home: '/tmp/cn-mock', mock: false })
  })
  it('uses a custom folder only when one is chosen and filled in', () => {
    expect(pluginOptions(chosen({ 'plugin.home': 'custom', 'plugin.home_path': ' /data/chatnip ' }))).toEqual({ home: '/data/chatnip', mock: false })
    expect(pluginOptions(chosen({ 'plugin.home': 'custom' }))).toEqual({ home: null, mock: false })
    expect(pluginOptions(chosen({ 'plugin.home': 'app', 'plugin.home_path': '/data/chatnip' }))).toEqual({ home: null, mock: false })
    expect(pluginHome(chosen({ 'plugin.home': 'elsewhere' }))).toBe('app')
  })
  it('offers install, update or reinstall from the installed state', () => {
    expect(pluginAction(null)).toBe('install')
    expect(pluginAction(cli({}))).toBe('install')
    expect(pluginAction(cli({ plugin_installed: true, plugin_version: '1-a' }))).toBe('reinstall')
    expect(pluginAction(cli({ plugin_installed: true, plugin_version: '0-z' }))).toBe('update')
  })
  it('prints the matching cli command', () => {
    expect(pluginCommand({ home: null, mock: false })).toBe('chatnip plugin install')
    expect(pluginCommand({ home: '~/.chatnip-mock', mock: true })).toBe('chatnip plugin install --home ~/.chatnip-mock --mock')
    expect(pluginCommand({ home: '/a b', mock: false })).toBe("chatnip plugin install --home '/a b'")
  })
})
