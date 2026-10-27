import type { Conversation, LocalFields, Person, User } from '$lib/tauri/types'
import { personInitials, personOf } from '$lib/tauri/people'

export type AvatarShape = 'person' | 'chat'

export interface AvatarFace {
  id: string
  name: string
  src: string | null
  color: string | null
}

export interface AvatarSource extends AvatarFace {
  shape: AvatarShape
  faces: AvatarFace[]
  icon: string | null
}

export const AVATAR_HUES = [18, 38, 58, 82, 108, 132, 152, 172, 192, 212, 232, 352] as const

export const AVATAR_FACES = 4

type Pictured = { id: string; display_name: string; avatar_data: string | null; avatar_url: string | null; overrides?: LocalFields }

export function avatarSeed(text: string): number {
  let hash = 0x811c9dc5
  for (const char of text) {
    hash ^= char.codePointAt(0) ?? 0
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return hash
}

export function avatarHue(seed: string): number {
  return AVATAR_HUES[avatarSeed(seed) % AVATAR_HUES.length]
}

export function avatarColor(face: Pick<AvatarFace, 'id' | 'color'>): string {
  return face.color?.trim() || `oklch(0.64 0.13 ${avatarHue(face.id)})`
}

export function avatarInitials(name: string): string {
  return personInitials(name)
}

export function imageSrc(data: string | null | undefined, url?: string | null): string | null {
  const raw = data?.trim()
  if (raw) return /^(data:|https?:|asset:|blob:)/.test(raw) ? raw : `data:image/png;base64,${raw}`
  return url?.trim() || null
}

export function personAvatar(person: Pictured): AvatarSource {
  return { shape: 'person', id: person.id, name: person.display_name, src: imageSrc(person.avatar_data, person.avatar_url), color: person.overrides?.color ?? null, faces: [], icon: null }
}

export function userAvatar(user: Pick<User, 'id' | 'display_name' | 'email' | 'avatar_url'> & { avatar_data?: string | null }, people: Person[], name?: string | null): AvatarSource {
  const person = personOf(people, { user_id: user.id })
  const label = name || person?.display_name || user.display_name || user.email || '?'
  if (person) return { ...personAvatar(person), name: label, src: personAvatar(person).src ?? imageSrc(user.avatar_data, user.avatar_url) }
  return { shape: 'person', id: user.id, name: label, src: imageSrc(user.avatar_data, user.avatar_url), color: null, faces: [], icon: null }
}

export function chatIcon(type: Conversation['type']): string | null {
  if (type === 'channel') return 'lucide:hash'
  if (type === 'email') return 'lucide:mail'
  return null
}

export function chatAvatar(conversation: Pick<Conversation, 'id' | 'name' | 'type' | 'image' | 'members' | 'overrides' | 'person_id'>, people: Person[], me: Set<string>): AvatarSource {
  const others = conversation.members.filter((member) => !me.has(member.id))
  const own = { shape: 'chat' as const, id: conversation.id, name: conversation.name, color: conversation.overrides?.color ?? null, faces: [], icon: null }
  if (conversation.image) return { ...own, src: imageSrc(conversation.image) }
  const owner = conversation.person_id ? people.find((person) => person.id === conversation.person_id) : null
  if (conversation.type === 'direct_message') {
    if (others[0]) return userAvatar(others[0], people)
    if (owner) return personAvatar(owner)
  }
  if (others.length >= 2) {
    const faces = [...new Map(others.map((member) => {
      const face = userAvatar(member, people)
      return [face.id, { id: face.id, name: face.name, src: face.src, color: face.color }] as const
    })).values()].slice(0, AVATAR_FACES)
    return { ...own, src: null, faces }
  }
  if (conversation.type === 'email' && owner) return personAvatar(owner)
  return { ...own, src: null, icon: chatIcon(conversation.type) }
}

export function imageData(source: Blob | string, max = 256): Promise<string> {
  return new Promise((resolve, reject) => {
    const url = typeof source === 'string' ? source : URL.createObjectURL(source)
    const image = new Image()
    image.crossOrigin = 'anonymous'
    image.onload = () => {
      const side = Math.min(image.naturalWidth || max, image.naturalHeight || max)
      const scale = Math.min(1, max / side)
      const size = Math.max(1, Math.round(side * scale))
      const canvas = document.createElement('canvas')
      canvas.width = size
      canvas.height = size
      const context = canvas.getContext('2d')
      if (!context) return reject(new Error('The image could not be read'))
      const sx = ((image.naturalWidth || side) - side) / 2
      const sy = ((image.naturalHeight || side) - side) / 2
      context.drawImage(image, sx, sy, side, side, 0, 0, size, size)
      if (typeof source !== 'string') URL.revokeObjectURL(url)
      resolve(canvas.toDataURL('image/png'))
    }
    image.onerror = () => {
      if (typeof source !== 'string') URL.revokeObjectURL(url)
      reject(new Error('The image could not be read'))
    }
    image.src = url
  })
}
