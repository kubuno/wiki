// Bridge so non-component callbacks (e.g. the host search bar's onSearch) know
// which wiki is currently open. The sidebar — mounted for every /wiki route —
// registers the active wiki id here.
// (Navigation itself no longer needs a bridge: use `navigate` from @kubuno/sdk.)
let activeWikiId: string | null = null

export function setActiveWiki(id: string | null) {
  activeWikiId = id
}

export function getActiveWiki(): string | null {
  return activeWikiId
}
