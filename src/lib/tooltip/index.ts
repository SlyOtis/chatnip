import { writable } from 'svelte/store';
export interface TooltipState {
  show: boolean;
  element: HTMLElement | null;
  text: string;
}
export const tooltipStore = writable<TooltipState>({
  show: false,
  element: null,
  text: '',
});
let showTimeout: number | null = null;
let currentElement: HTMLElement | null = null;
function cleanup() {
  if (showTimeout) {
    clearTimeout(showTimeout);
    showTimeout = null;
  }
  if (currentElement) {
    currentElement.removeEventListener('mouseleave', hideTooltip);
    currentElement.removeEventListener('click', hideTooltip);
    currentElement = null;
  }
}
export function hideTooltip() {
  tooltipStore.set({ show: false, element: null, text: '' });
  cleanup();
}
export function showTooltip(element: HTMLElement, text: string) {
  cleanup();
  currentElement = element;
  element.addEventListener('mouseleave', hideTooltip);
  element.addEventListener('click', hideTooltip);
  showTimeout = setTimeout(() => {
    if (currentElement === element) {
      tooltipStore.set({ show: true, element, text });
    }
  }, 300);
}
function handleGlobalClick(e: MouseEvent) {
  if (currentElement && !currentElement.contains(e.target as Node)) {
    hideTooltip();
  }
}
function handleGlobalScroll() {
  hideTooltip();
}
if (typeof window !== 'undefined') {
  document.addEventListener('click', handleGlobalClick);
  window.addEventListener('scroll', handleGlobalScroll, true);
  window.addEventListener('beforeunload', cleanup);
}
