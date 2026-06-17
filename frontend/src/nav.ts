// Bridge so non-component callbacks (e.g. the host search bar's onSearch) can use
// the module's react-router navigation. The sidebar — mounted for every /wiki
// route — registers the live navigate function here, plus the active wiki id.
let navFn: ((to: string) => void) | null = null
let activeWikiId: string | null = null

export function setNav(fn: (to: string) => void) {
  navFn = fn
}

export function goTo(to: string) {
  navFn?.(to)
}

export function setActiveWiki(id: string | null) {
  activeWikiId = id
}

export function getActiveWiki(): string | null {
  return activeWikiId
}
