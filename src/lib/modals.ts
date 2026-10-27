import { type Readable, writable } from 'svelte/store'
import type {
  ChannelBrowserModalData,
  DetailsModalData,
  DiscussModalData,
  EmailComposeModalData,
  ImageModalData,
  ModalsState,
  SearchModalData,
  ShortcutsModalData,
  StatusModalData,
  ImageUploadModalData
} from './types'
export const modalsStore = writable<ModalsState>({
  search: null,
  image: null,
  details: null,
  imageUpload: null,
  emailCompose: null,
  discuss: null,
  setStatus: null,
  shortcuts: null,
  channelBrowser: null,
  isOpen: false,
})
export const modalsShow = (search: SearchModalData | ImageModalData | DetailsModalData | ImageUploadModalData | EmailComposeModalData | DiscussModalData | StatusModalData | ShortcutsModalData | ChannelBrowserModalData): Readable<ModalsState> => {
  let data: ModalsState = {
    search: null,
    image: null,
    details: null,
      imageUpload: null,
    emailCompose: null,
    discuss: null,
    setStatus: null,
    shortcuts: null,
    channelBrowser: null,
    isOpen: false,
  }
  if ('query' in search ) {
    data.search = search
  }
  if ('source' in search ) {
    data.image = search
  }
  if ('conversation_id' in search && 'message_id' in search) {
    data.details = search
  }
  if ('currentImage' in search || 'newImage' in search) {
    data.imageUpload = search
  }
  if ('mode' in search) {
    data.emailCompose = search as EmailComposeModalData
  }
  if ('message' in search && 'conversation_id' in search && !('message_id' in search)) {
    data.discuss = search as DiscussModalData
  }
  if ('initial_emoji' in search || 'initial_text' in search) {
    data.setStatus = search as StatusModalData
  }
  if ('shortcuts' in search) {
    data.shortcuts = search as ShortcutsModalData
  }
  if ('browseChannels' in search) {
    data.channelBrowser = search as ChannelBrowserModalData
  }
  modalsStore.set({
    ...data,
    isOpen: true,
  })
  return modalsStore
}

export const modalsClose = () => {
  modalsStore.update(store => ({
    ...store,
    isOpen: false,
  }))
}
