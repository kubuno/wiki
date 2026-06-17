import { create } from 'zustand'

interface WikiState {
  searchQuery: string
  setSearchQuery: (q: string) => void
}

export const useWikiStore = create<WikiState>((set) => ({
  searchQuery: '',
  setSearchQuery: (q) => set({ searchQuery: q }),
}))
