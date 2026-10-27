<script lang="ts">
  import { modalsStore } from '$lib/modals'
  import { forwarding } from '$lib/reply.svelte'
  import ForwardModal from './ForwardModal.svelte'
  
  let modalCom = $state<any>()
  let modalProps = $state<any>()
  let isOpen = $derived($modalsStore.isOpen)
  let modalType = $derived.by(() => {
    if ($modalsStore.search) {
      return 'search'
    }
    if ($modalsStore.image) {
      return 'image'
    }
    if ($modalsStore.details) {
      return 'details'
    }
    if ($modalsStore.imageUpload) {
      return 'imageUpload'
    }
    if ($modalsStore.emailCompose) {
      return 'emailCompose'
    }
    if ($modalsStore.discuss) {
      return 'discuss'
    }
    if ($modalsStore.setStatus) {
      return 'setStatus'
    }
    if ($modalsStore.shortcuts) {
      return 'shortcuts'
    }
    if ($modalsStore.channelBrowser) {
      return 'channelBrowser'
    }
    return undefined
  })
  
  $effect(() => {
    if (!modalType) {
      modalCom = null
      modalProps = null
      return
    }
    
    switch (modalType) {
      case 'search':
        modalProps = $modalsStore.search
        import('./SearchModal.svelte')
          .then(res => modalCom = res.default)
        break
      case 'image':
        modalProps = $modalsStore.image
        import('./ImageModal.svelte')
          .then(res => modalCom = res.default)
        break
      case 'imageUpload':
        modalProps = $modalsStore.imageUpload
        import('./ImageUploadModal.svelte')
          .then(res => modalCom = res.default)
        break
      case 'emailCompose':
        modalProps = $modalsStore.emailCompose
        import('./EmailComposeModal.svelte')
          .then(res => modalCom = res.default)
        break
      case 'discuss':
        modalProps = $modalsStore.discuss
        import('./DiscussModal.svelte')
          .then(res => modalCom = res.default)
        break
      case 'setStatus':
        modalProps = $modalsStore.setStatus
        import('./SetStatusModal.svelte')
          .then(res => modalCom = res.default)
        break
      case 'shortcuts':
        modalProps = $modalsStore.shortcuts
        import('./ShortcutsModal.svelte')
          .then(res => modalCom = res.default)
        break
      case 'channelBrowser':
        modalProps = $modalsStore.channelBrowser
        import('./ChannelBrowserModal.svelte')
          .then(res => modalCom = res.default)
        break
    }
  })
</script>
{#if modalCom && modalProps && isOpen}
    {@const Modal = modalCom}
    <Modal {...modalProps} />
{/if}
{#if forwarding.messages.length}
  <ForwardModal />
{/if}
