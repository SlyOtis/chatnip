import type { Message } from '../types'
import type en from '../en/composer'

export default {
  'composer.message': 'Melding',
  'composer.link': 'Lenkeadresse',
  'composer.markdownSource': 'Markdown-kilde',
  'composer.channel.mail': 'E-post',
  'composer.channel.teamsGroup': 'Teams-gruppe',
  'composer.channel.teamsChat': 'Teams-chat',
  'composer.channel.slackChannel': 'Slack-kanal',
  'composer.channel.slackDm': 'Slack-DM',
  'composer.channel.chat': 'Chat',
  'composer.via.newMail': 'Ny e-post til {address}',
  'composer.via.newChat': 'Ny chat med {name}'
} satisfies Record<keyof typeof en, Message>
