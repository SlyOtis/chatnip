import type { MessageKey } from './en'
import type { Message } from './types'
import common from './nb/common'
import app from './nb/app'
import setup from './nb/setup'
import settings from './nb/settings'
import conversation from './nb/conversation'
import composer from './nb/composer'
import modals from './nb/modals'
import inbox from './nb/inbox'
import calendar from './nb/calendar'
import cases from './nb/cases'
import actions from './nb/actions'
import stats from './nb/stats'
import people from './nb/people'
import dates from './nb/dates'
import content from './nb/content'
import chats from './nb/chats'
import voice from './nb/voice'

export const nb: Record<MessageKey, Message> = {
  ...common,
  ...app,
  ...setup,
  ...settings,
  ...conversation,
  ...composer,
  ...modals,
  ...inbox,
  ...calendar,
  ...cases,
  ...actions,
  ...stats,
  ...people,
  ...dates,
  ...content,
  ...chats,
  ...voice
}
