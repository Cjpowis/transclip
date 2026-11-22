import { useSyncExternalStore } from "react"

/** Which instructions to show: a Mac's, or Windows'. */
export type Platform = "mac" | "windows"

// Windows visitors see Windows first; everyone can switch.
let current: Platform =
  typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent)
    ? "windows"
    : "mac"
const listeners = new Set<() => void>()

export function setPlatform(next: Platform) {
  current = next
  listeners.forEach((listener) => listener())
}

export function usePlatform(): Platform {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => current,
    () => "mac",
  )
}
