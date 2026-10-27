export function debounce<T extends (..._args: unknown[]) => unknown>(
  func: T,
  wait: number
): (..._args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout> | undefined
  return function debounced(...args: Parameters<T>) {
    const later = () => {
      timeoutId = undefined
      func(...args)
    }
    clearTimeout(timeoutId)
    timeoutId = setTimeout(later, wait)
  }
}