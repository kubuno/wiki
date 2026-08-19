import { useQuery } from '@tanstack/react-query'
import { api, useAuthStore } from '@kubuno/sdk'

// Instance policy on who may open a space, as the administrator left it in the
// console. The BACKEND is what enforces it (wikis::create returns 403); this
// only stops the interface from offering an action it already knows will be
// refused — a button that always fails is worse than no button.
//
// The two keys are declared `public` in wiki's module.toml, which is what puts
// them in `/api/v1/config` under `wiki.<key>`, the only config route a non-admin
// may read. A missing key falls back to the permissive compiled default, so an
// older core never locks anyone out.

type Policy = 'everyone' | 'admins'

export interface WikiCreationPolicy {
  /** May this user open a space at all? */
  canCreate:       boolean
  /** May this user open a SHARED space (one others are invited into)? */
  canCreateShared: boolean
}

function readPolicy(value: unknown): Policy {
  return value === 'admins' ? 'admins' : 'everyone'
}

export function useWikiCreationPolicy(): WikiCreationPolicy {
  const isAdmin = useAuthStore(s => s.user?.role === 'admin')

  const { data } = useQuery({
    queryKey: ['wiki-instance-config'],
    queryFn: async () => {
      const res = await api.get<{ config: Record<string, unknown> }>('/config')
      return res.data.config ?? {}
    },
    staleTime: 5 * 60_000,
  })
  const cfg = data ?? {}
  const allows = (key: string) => readPolicy(cfg[key]) === 'everyone' || isAdmin

  return {
    canCreate:       allows('wiki.wiki_creation'),
    canCreateShared: allows('wiki.shared_wiki_creation'),
  }
}
