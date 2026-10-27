import type { RuleCondition, RuleField, RuleOperator } from '$lib/tauri/types'
import { t, type MessageKey } from '$lib/i18n.svelte'

export const FIELDS: { value: RuleField; label: MessageKey }[] = [
  { value: 'sender_email', label: 'settings.rules.field.senderEmail' },
  { value: 'sender_domain', label: 'settings.rules.field.senderDomain' },
  { value: 'sender_name', label: 'settings.rules.field.senderName' },
  { value: 'subject', label: 'settings.rules.field.subject' },
  { value: 'body', label: 'settings.rules.field.body' },
  { value: 'conversation_name', label: 'settings.rules.field.conversationName' },
  { value: 'conversation_type', label: 'settings.rules.field.conversationType' },
  { value: 'provider_type', label: 'settings.rules.field.provider' },
  { value: 'connection_name', label: 'settings.rules.field.connection' },
  { value: 'namespace', label: 'settings.rules.field.namespace' },
  { value: 'member_count', label: 'settings.rules.field.memberCount' }
]

export const OPERATORS: { value: RuleOperator; label: MessageKey }[] = [
  { value: 'is', label: 'settings.rules.operator.is' },
  { value: 'is_not', label: 'settings.rules.operator.isNot' },
  { value: 'contains', label: 'settings.rules.operator.contains' },
  { value: 'not_contains', label: 'settings.rules.operator.notContains' },
  { value: 'starts_with', label: 'settings.rules.operator.startsWith' },
  { value: 'ends_with', label: 'settings.rules.operator.endsWith' },
  { value: 'matches', label: 'settings.rules.operator.matches' },
  { value: 'greater_than', label: 'settings.rules.operator.greaterThan' },
  { value: 'less_than', label: 'settings.rules.operator.lessThan' }
]

export function emptyCondition(): RuleCondition {
  return { field: 'sender_domain', operator: 'ends_with', value: '' }
}

export function regexError(condition: RuleCondition): string | null {
  if (condition.operator !== 'matches' || !condition.value) return null
  try {
    new RegExp(condition.value)
    return null
  } catch (error) {
    return error instanceof Error ? error.message : String(error)
  }
}

function fieldLabel(field: RuleField): string {
  const label = FIELDS.find((f) => f.value === field)?.label
  return label ? t(label).toLowerCase() : field
}

function operatorLabel(operator: RuleOperator): string {
  const label = OPERATORS.find((o) => o.value === operator)?.label
  return label ? t(label) : operator
}

export function describeConditions(conditions: RuleCondition[], match: 'all' | 'any'): string {
  if (conditions.length === 0) return t('settings.rules.everything')
  return conditions
    .map((c) => t('settings.rules.condition', { field: fieldLabel(c.field), operator: operatorLabel(c.operator), value: c.operator === 'matches' ? `/${c.value}/` : c.value }))
    .join(t(match === 'all' ? 'settings.rules.and' : 'settings.rules.or'))
}
