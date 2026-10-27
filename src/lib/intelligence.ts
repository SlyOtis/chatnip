import type { Action, ActionGraph, ClaudeStatus, CliSetup, IntelligenceConnection, IntelligenceKind, ModelFeature, OllamaStatus, PluginOptions, PostIntelligenceConnectionInput, PullProgress, SettingRegistry } from '$lib/tauri/types'
import type { Choice } from '$lib/components/ui/IconPicker.svelte'
import { formatNumber, t } from '$lib/i18n.svelte'
import { settingValue } from '$lib/settings'
import { BUILTIN } from '$lib/actions'

export interface ConfigField {
  key: string
  label: string
  help: string
  secret?: boolean
  placeholder?: string
  options?: string[]
}

export const kinds: { kind: IntelligenceKind; label: string; icon: string; summary: string }[] = [
  { kind: 'claude_cli', label: 'Claude Code', icon: 'lucide:terminal', get summary() { return t('actions.origin.claudeSummary') } },
  { kind: 'ollama', label: 'Ollama', icon: 'lucide:cpu', get summary() { return t('actions.origin.ollamaSummary') } },
  { kind: 'replicate', label: 'Replicate', icon: 'lucide:cloud', get summary() { return t('actions.origin.replicateSummary') } },
  { kind: 'openai_compatible', get label() { return t('actions.origin.openai') }, icon: 'lucide:server', get summary() { return t('actions.origin.openaiSummary') } },
  { kind: 'command', get label() { return t('actions.origin.command') }, icon: 'lucide:square-terminal', get summary() { return t('actions.origin.commandSummary') } }
]

export function kindLabel(kind: IntelligenceKind): string {
  return kinds.find((k) => k.kind === kind)?.label ?? kind
}

export function kindIcon(kind: IntelligenceKind): string {
  return kinds.find((k) => k.kind === kind)?.icon ?? 'lucide:sparkles'
}

export function configFields(kind: IntelligenceKind): ConfigField[] {
  switch (kind) {
    case 'claude_cli':
      return [
        { key: 'binary', label: t('actions.field.binary'), help: t('actions.help.claudeCli.binary'), placeholder: 'claude' },
        { key: 'model', label: t('actions.field.model'), help: t('actions.help.claudeCli.model') }
      ]
    case 'ollama':
      return [
        { key: 'host', label: t('actions.field.host'), help: t('actions.help.ollama.host'), placeholder: 'http://localhost:11434' },
        { key: 'model', label: t('actions.field.model'), help: t('actions.help.ollama.model') }
      ]
    case 'replicate':
      return [
        { key: 'api_token', label: t('actions.field.apiToken'), help: t('actions.help.replicate.apiToken'), secret: true, placeholder: 'r8_…' },
        { key: 'model', label: t('actions.field.model'), help: t('actions.help.replicate.model'), placeholder: 'anthropic/claude-4-sonnet' }
      ]
    case 'openai_compatible':
      return [
        { key: 'base_url', label: t('actions.field.baseUrl'), help: t('actions.help.openaiCompatible.baseUrl'), placeholder: 'http://localhost:1234' },
        { key: 'api_key', label: t('actions.field.apiKey'), help: t('actions.help.openaiCompatible.apiKey'), secret: true },
        { key: 'model', label: t('actions.field.model'), help: t('actions.help.openaiCompatible.model'), placeholder: 'llama3' }
      ]
    case 'command':
      return [
        { key: 'command', label: t('actions.field.command'), help: t('actions.help.command.command'), placeholder: 'llm' },
        { key: 'args', label: t('actions.field.args'), help: t('actions.help.command.args'), placeholder: '--model {{ .model }} --json' },
        { key: 'stdin', label: t('actions.field.stdin'), help: t('actions.help.command.stdin'), options: ['prompt', 'none'] },
        { key: 'output', label: t('actions.field.output'), help: t('actions.help.command.output'), options: ['json', 'text'] },
        { key: 'model_arg', label: t('actions.field.modelArg'), help: t('actions.help.command.modelArg'), placeholder: '--model' },
        { key: 'models_command', label: t('actions.field.modelsCommand'), help: t('actions.help.command.modelsCommand'), placeholder: 'llm models' }
      ]
  }
}

export function emptyConfig(kind: IntelligenceKind): Record<string, string> {
  const config: Record<string, string> = {}
  for (const field of configFields(kind)) config[field.key] = field.options ? field.options[0] : ''
  return config
}

export function templateVariables(template: string): string[] {
  const found = new Set<string>()
  for (const match of template.matchAll(/\{\{-?\s*\.([A-Za-z_][\w.]*)[^}]*\}\}/g)) {
    found.add(match[1].split('.')[0])
  }
  return [...found]
}

export function missingVariables(template: string, available: string[]): string[] {
  return templateVariables(template).filter((name) => !available.includes(name))
}

export function insertVariable(template: string, name: string, cursor: number): { template: string; cursor: number } {
  const token = `{{ .${name} }}`
  const at = Math.max(0, Math.min(cursor, template.length))
  return { template: template.slice(0, at) + token + template.slice(at), cursor: at + token.length }
}

export function originName(prompt: { connection_id: string | null }, connections: IntelligenceConnection[]): string {
  const chosen = prompt.connection_id ? connections.find((c) => c.id === prompt.connection_id) : undefined
  if (chosen) return chosen.name
  const fallback = connections.find((c) => c.is_default)
  return fallback ? t('actions.origin.default', { name: fallback.name }) : t('actions.origin.none')
}

export function formatCost(value: number): string {
  return value > 0 ? `$${formatNumber(value, { minimumFractionDigits: 4, maximumFractionDigits: 4 })}` : t('actions.cost.free')
}

export function formatDuration(ms: number): string {
  return ms >= 1000 ? `${formatNumber(ms / 1000, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} s` : `${formatNumber(ms)} ms`
}

export function parseSchema(text: string): string | null {
  const trimmed = text.trim()
  if (!trimmed) return null
  JSON.parse(trimmed)
  return trimmed
}

export function formatBytes(bytes: number): string {
  if (!bytes) return ''
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000
    unit += 1
  }
  return `${value >= 10 || unit === 0 ? formatNumber(Math.round(value)) : formatNumber(value, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} ${units[unit]}`
}

export function pullPercent(progress: Pick<PullProgress, 'completed' | 'total'> | null | undefined): number | null {
  if (!progress?.total || progress.completed == null) return null
  return Math.min(100, Math.max(0, Math.round((progress.completed / progress.total) * 100)))
}

export function localConnection(connections: IntelligenceConnection[], kind: 'claude_cli' | 'ollama'): IntelligenceConnection | undefined {
  return connections.find((c) => c.kind === kind)
}

export function missingModel(model: string | null | undefined, installed: string[]): boolean {
  const name = model?.trim() ?? ''
  return !installed.some((m) => m === name || m === `${name}:latest`)
}

export function claudeConnectionInput(status: ClaudeStatus, model: string): PostIntelligenceConnectionInput {
  const config: Record<string, string> = { model }
  if (status.binary) config.binary = status.binary
  return { kind: 'claude_cli', name: 'Claude Code', config }
}

export function ollamaConnectionInput(status: OllamaStatus, model: string): PostIntelligenceConnectionInput {
  return { kind: 'ollama', name: 'Ollama', config: { host: status.host, model: model || status.models[0]?.name || '' } }
}

export function ollamaStep(status: OllamaStatus): 'install' | 'start' | 'pull' | 'ready' {
  if (!status.installed) return 'install'
  if (!status.running) return 'start'
  if (status.models.length === 0) return 'pull'
  return 'ready'
}

export type FeaturePart = 'connection' | 'model'

export function featureKey(feature: string, part: FeaturePart): string {
  return `ai.feature.${feature}.${part}`
}

export interface ModelSelection {
  connection: string
  model: string
}

export interface ModelDefaults {
  models: ModelFeature['models']
  local: boolean
}

export const LOCAL_NODE = `${BUILTIN}receipt_confirm`

export function featureSelection(feature: string, values: Record<string, string>): ModelSelection {
  return { connection: values[featureKey(feature, 'connection')]?.trim() ?? '', model: values[featureKey(feature, 'model')]?.trim() ?? '' }
}

export function actionSelection(action: Pick<Action, 'ai'>): ModelSelection {
  return { connection: action.ai.connection_id?.trim() ?? '', model: action.ai.model?.trim() ?? '' }
}

export function actionDefaults(graph: ActionGraph): ModelDefaults {
  return { models: [], local: graph.nodes.some((n) => n.type === LOCAL_NODE) }
}

export interface ModelPicker {
  connection: IntelligenceConnection | undefined
  value: string
  label: string
  defaultLabel: string
  own: boolean
  missing: boolean
}

export function modelChoice(connectionId: string, model: string): string {
  return `${connectionId}|${model}`
}

export function parseModelChoice(value: string): ModelSelection {
  const at = value.indexOf('|')
  return at < 0 ? { connection: value, model: '' } : { connection: value.slice(0, at), model: value.slice(at + 1) }
}

function ownModel(connection: IntelligenceConnection, claude: string | null): string {
  const own = connection.config.model?.trim() ?? ''
  if (own) return own
  if (connection.kind !== 'claude_cli') return ''
  return claude ? t('settings.ai.cliDefaultModel', { model: claude }) : t('settings.ai.cliDefault')
}

export function modelChoices(connections: IntelligenceConnection[], claude: string | null = null): Choice[] {
  return connections.flatMap((c) => {
    const own = c.config.model?.trim() ?? ''
    const first = own || c.kind === 'claude_cli' ? [{ value: modelChoice(c.id, ''), label: `${c.name} · ${ownModel(c, claude)}`, group: c.name }] : []
    const rest = [...new Set(c.models.filter((m) => !!m && m !== own))].map((m) => ({ value: modelChoice(c.id, m), label: `${c.name} · ${m}`, group: c.name }))
    return [...first, ...rest]
  })
}

export function modelPicker(selection: ModelSelection, connections: IntelligenceConnection[], defaults: ModelDefaults, claude: string | null = null): ModelPicker {
  const pinned = selection.connection.trim()
  const chosen = connections.find((c) => c.id === pinned)
  const connection = chosen ?? (defaults.local ? connections.find((c) => c.kind === 'ollama') : undefined) ?? connections.find((c) => c.is_default) ?? connections[0]
  const model = selection.model.trim()
  const own = !!pinned || !!model
  const connectionModel = connection?.config.model?.trim() ?? ''
  const preset = defaults.models.find(([kind]) => kind === connection?.kind)?.[1] ?? ''
  const smallest = defaults.local && connection?.kind === 'ollama' && !preset
  const missing = (!!pinned && !chosen) || (!!model && !!connection && connection.models.length > 0 && ![...connection.models, connectionModel].includes(model))
  const shown = connection ? `${connection.name} · ${preset || (smallest ? t('settings.models.smallest') : ownModel(connection, claude))}` : ''
  const defaultLabel = connection ? t('settings.models.useDefault', { choice: shown }) : t('settings.models.none')
  const label = own && connection ? `${connection.name} · ${model || ownModel(connection, claude)}` : defaultLabel
  return { connection, value: own && connection && (chosen || !pinned) ? modelChoice(connection.id, model) : '', label, defaultLabel, own, missing }
}

export const PLUGIN_HOME = 'plugin.home'
export const PLUGIN_HOME_PATH = 'plugin.home_path'
export const PLUGIN_MOCK = 'plugin.mock'
export const MOCK_HOME = 'mock.home'

export type PluginHome = 'app' | 'mock' | 'custom'

export const PLUGIN_HOMES: readonly PluginHome[] = ['app', 'mock', 'custom']

type Registry = Pick<SettingRegistry, 'defaults' | 'values'> | null | undefined

export function pluginHome(registry: Registry): PluginHome {
  const value = settingValue(registry, PLUGIN_HOME)
  return PLUGIN_HOMES.find((home) => home === value) ?? 'app'
}

export function pluginOptions(registry: Registry): PluginOptions {
  const home = pluginHome(registry)
  const path = home === 'mock' ? settingValue(registry, MOCK_HOME) : home === 'custom' ? settingValue(registry, PLUGIN_HOME_PATH) : ''
  return { home: path.trim() || null, mock: settingValue(registry, PLUGIN_MOCK) === 'true' }
}

export function pluginAction(cli: CliSetup | null): 'install' | 'update' | 'reinstall' {
  if (!cli?.plugin_installed) return 'install'
  return cli.plugin_version === cli.plugin_current ? 'reinstall' : 'update'
}

export function pluginCommand(options: PluginOptions): string {
  return ['chatnip plugin install', options.home ? `--home ${/\s/.test(options.home) ? `'${options.home}'` : options.home}` : '', options.mock ? '--mock' : ''].filter((part) => !!part).join(' ')
}
