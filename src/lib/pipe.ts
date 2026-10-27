import { accepts, NODE_CHAT } from '$lib/actions'
import { t } from '$lib/i18n.svelte'
import type { ActionPort, ChatRoute, LandingNode, PipeStep, RouteProblem } from '$lib/tauri/types'

export function defaultRoutes(outputs: ActionPort[]): ChatRoute[] {
  return outputs.filter((p) => p.type === 'list' || p.type === 'json').map((p) => ({ output: p.name, node: NODE_CHAT, input: p.type === 'list' ? 'items' : 'item', values: {} }))
}

export function routeProblem(outputs: ActionPort[], landings: LandingNode[], route: ChatRoute): string | null {
  const output = outputs.find((p) => p.name === route.output)
  if (!output) return t('chats.pipe.noOutput', { output: route.output })
  const landing = landings.find((l) => l.node === route.node)
  if (!landing) return t('chats.pipe.noLanding', { node: route.node })
  const input = landing.inputs.find((p) => p.name === route.input)
  if (!input) return t('chats.pipe.noInput', { landing: landing.label, input: route.input })
  const each = output.type === 'list' && !['list', 'json', 'any'].includes(input.type)
  if (!each && !accepts(input.type, output.type)) return t('chats.pipe.mismatch', { output: output.name, outputType: output.type, landing: landing.label, inputType: input.type, input: input.name })
  return null
}

export function validateRoutes(outputs: ActionPort[], landings: LandingNode[], routes: ChatRoute[]): RouteProblem[] {
  return routes.flatMap((route, index) => {
    const message = routeProblem(outputs, landings, route)
    return message ? [{ index, message }] : []
  })
}

export function fittingInputs(landing: LandingNode | undefined, output: ActionPort | undefined): ActionPort[] {
  if (!landing) return []
  if (!output) return landing.inputs
  return landing.inputs.filter((input) => accepts(input.type, output.type) || (output.type === 'list' && !['list', 'json', 'any'].includes(input.type)))
}

export function newRoute(outputs: ActionPort[], landings: LandingNode[]): ChatRoute {
  const output = outputs[0]
  const landing = landings[0]
  return { output: output?.name ?? '', node: landing?.node ?? NODE_CHAT, input: fittingInputs(landing, output)[0]?.name ?? landing?.inputs[0]?.name ?? '', values: {} }
}

export function changeRoute(route: ChatRoute, patch: Partial<ChatRoute>, outputs: ActionPort[], landings: LandingNode[]): ChatRoute {
  const next = { ...route, ...patch }
  if (patch.node !== undefined || patch.output !== undefined) {
    const fits = fittingInputs(landings.find((l) => l.node === next.node), outputs.find((p) => p.name === next.output))
    if (!fits.some((p) => p.name === next.input)) next.input = fits[0]?.name ?? ''
    if (patch.node !== undefined) next.values = {}
  }
  return next
}

export interface StepState {
  step: PipeStep
  routes: ChatRoute[]
  problems: RouteProblem[]
  dirty: boolean
  lands: LandingNode[]
}

export function stepState(step: PipeStep, landings: LandingNode[], edited: ChatRoute[] | undefined): StepState {
  const routes = edited ?? step.routes
  const problems = validateRoutes(step.outputs, landings, routes)
  const lands = [...new Set(routes.map((r) => r.node))].map((node) => landings.find((l) => l.node === node)).filter((l): l is LandingNode => !!l)
  return { step, routes, problems, dirty: !!edited && JSON.stringify(edited) !== JSON.stringify(step.routes), lands }
}

export function extraInputs(landing: LandingNode | undefined, route: ChatRoute): ActionPort[] {
  const fromPayload = ['message', 'conversation', 'person', 'text', 'item', 'items']
  return (landing?.inputs ?? []).filter((p) => p.name !== route.input && !fromPayload.includes(p.name))
}
