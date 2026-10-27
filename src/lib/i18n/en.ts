import common from './en/common'
import app from './en/app'
import setup from './en/setup'
import settings from './en/settings'
import conversation from './en/conversation'
import composer from './en/composer'
import modals from './en/modals'
import inbox from './en/inbox'
import calendar from './en/calendar'
import cases from './en/cases'
import actions from './en/actions'
import stats from './en/stats'
import people from './en/people'
import dates from './en/dates'
import content from './en/content'
import chats from './en/chats'
import voice from './en/voice'

export const en = {
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

export type MessageKey = keyof typeof en
