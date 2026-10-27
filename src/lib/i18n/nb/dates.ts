import type { Message } from '../types'
import type en from '../en/dates'

export default {
  'dates.yesterdayAt': 'I går {time}'
} satisfies Record<keyof typeof en, Message>
