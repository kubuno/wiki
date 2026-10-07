import { api as apiClient } from '@kubuno/sdk'

// ── Types ────────────────────────────────────────────────────────────────────

export interface Wiki {
  id: string
  owner_id: string
  slug: string
  name: string
  description: string
  is_shared: boolean
  created_at: string
  updated_at: string
  my_role: 'owner' | 'admin' | 'editor' | 'reader' | 'none'
  page_count: number
}

export interface PageSummary {
  id: string
  namespace: string
  title: string
  slug: string
  redirect_to: string | null
  preview: string
  byte_size: number
  current_rev_at: string
}

export interface TocEntry { level: number; text: string; id: string }
export interface CategoryRef { title: string; slug: string }

export interface PageResponse {
  exists: boolean
  id?: string
  namespace: string
  title: string
  slug: string
  prefixed_title: string
  talk_namespace: string | null
  redirect?: string | null
  html: string
  toc?: TocEntry[]
  categories: CategoryRef[]
  source: string
  updated_at?: string
  can_edit: boolean
  can_admin: boolean
}

export interface PreviewResponse {
  html: string
  toc: TocEntry[]
  categories: CategoryRef[]
  redirect: string | null
}

export interface Member {
  user_id: string
  role: string
  display_name: string
  email: string
  added_at: string
}

export interface Revision {
  rev_id: string
  author_id: string | null
  author_name: string
  ts: string
  comment: string
  minor: boolean
  size: number
}

export interface RecentChange {
  id: string
  page_id: string | null
  namespace: string
  title: string
  author_name: string
  comment: string
  minor: boolean
  change_type: 'create' | 'edit' | 'delete' | 'move'
  byte_delta: number
  created_at: string
}

export interface RecentPage { wiki_id: string; namespace: string; title: string; slug: string; current_rev_at: string }
export interface WantedPage { namespace: string; title: string; slug: string; refs: number }
export interface CategoryCount { title: string; slug: string; pages: number }
export interface SearchHit { id: string; namespace: string; title: string; slug: string; preview: string; rank: number }
export interface Backlink { id: string; namespace: string; title: string; slug: string }

const base = '/wiki'

// ── API ──────────────────────────────────────────────────────────────────────

export const wikiApi = {
  listWikis: () => apiClient.get<{ wikis: Wiki[] }>(`${base}/wikis`).then(r => r.data.wikis),
  createWiki: (data: { name: string; description?: string; is_shared?: boolean }) =>
    apiClient.post<{ wiki: Wiki }>(`${base}/wikis`, data).then(r => r.data.wiki),
  getWiki: (id: string) => apiClient.get<{ wiki: Wiki }>(`${base}/wikis/${id}`).then(r => r.data.wiki),
  updateWiki: (id: string, data: { name?: string; description?: string }) =>
    apiClient.patch<{ wiki: Wiki }>(`${base}/wikis/${id}`, data).then(r => r.data.wiki),
  deleteWiki: (id: string) => apiClient.delete(`${base}/wikis/${id}`),

  listMembers: (id: string) => apiClient.get<{ members: Member[] }>(`${base}/wikis/${id}/members`).then(r => r.data.members),
  addMember: (id: string, data: { email: string; role: string }) => apiClient.post(`${base}/wikis/${id}/members`, data),
  updateMember: (id: string, memberId: string, role: string) => apiClient.patch(`${base}/wikis/${id}/members/${memberId}`, { role }),
  removeMember: (id: string, memberId: string) => apiClient.delete(`${base}/wikis/${id}/members/${memberId}`),

  listPages: (id: string) => apiClient.get<{ pages: PageSummary[] }>(`${base}/wikis/${id}/pages`).then(r => r.data.pages),
  getPage: (id: string, ns: string, title: string) =>
    apiClient.get<PageResponse>(`${base}/wikis/${id}/page`, { params: { ns, title } }).then(r => r.data),
  savePage: (id: string, data: { namespace?: string; title: string; content: string; comment?: string; minor?: boolean }) =>
    apiClient.post<{ page: PageSummary }>(`${base}/wikis/${id}/page`, data).then(r => r.data.page),
  previewPage: (id: string, data: { namespace?: string; title: string; content: string }) =>
    apiClient.post<PreviewResponse>(`${base}/wikis/${id}/page/preview`, data).then(r => r.data),
  deletePage: (id: string, pageId: string) => apiClient.delete(`${base}/wikis/${id}/pages/${pageId}`),
  movePage: (id: string, pageId: string, target: string) =>
    apiClient.post<{ page: PageSummary }>(`${base}/wikis/${id}/pages/${pageId}/move`, { target }).then(r => r.data.page),
  history: (id: string, pageId: string) =>
    apiClient.get<{ revisions: Revision[] }>(`${base}/wikis/${id}/pages/${pageId}/history`).then(r => r.data.revisions),
  revision: (id: string, pageId: string, revId: string) =>
    apiClient.get<{ revision: Revision & { content: string } }>(`${base}/wikis/${id}/pages/${pageId}/revisions/${revId}`).then(r => r.data.revision),
  backlinks: (id: string, pageId: string) =>
    apiClient.get<{ backlinks: Backlink[] }>(`${base}/wikis/${id}/pages/${pageId}/backlinks`).then(r => r.data.backlinks),
  openByFile: (fileId: string) =>
    apiClient.post<{ wiki_id: string; namespace: string; title: string }>(`${base}/open-by-file`, { file_id: fileId }).then(r => r.data),
  recentPages: (limit = 12) =>
    apiClient.get<{ pages: RecentPage[] }>(`${base}/recent`, { params: { limit } }).then(r => r.data.pages),

  allPages: (id: string, ns?: string) =>
    apiClient.get<{ pages: PageSummary[] }>(`${base}/wikis/${id}/special/allpages`, { params: ns ? { ns } : {} }).then(r => r.data.pages),
  recentChanges: (id: string, limit = 100) =>
    apiClient.get<{ changes: RecentChange[] }>(`${base}/wikis/${id}/special/recentchanges`, { params: { limit } }).then(r => r.data.changes),
  wantedPages: (id: string) =>
    apiClient.get<{ pages: WantedPage[] }>(`${base}/wikis/${id}/special/wantedpages`).then(r => r.data.pages),
  orphanedPages: (id: string) =>
    apiClient.get<{ pages: PageSummary[] }>(`${base}/wikis/${id}/special/orphaned`).then(r => r.data.pages),
  categories: (id: string) =>
    apiClient.get<{ categories: CategoryCount[] }>(`${base}/wikis/${id}/special/categories`).then(r => r.data.categories),
  categoryMembers: (id: string, slug: string) =>
    apiClient.get<{ pages: PageSummary[] }>(`${base}/wikis/${id}/category/${slug}`).then(r => r.data.pages),
  search: (id: string, q: string, limit = 30) =>
    apiClient.get<{ results: SearchHit[] }>(`${base}/wikis/${id}/search`, { params: { q, limit } }).then(r => r.data.results),
}

// ── Helpers ──────────────────────────────────────────────────────────────────

export const SPECIAL_PAGES = [
  { name: 'allpages',      labelKey: 'sp_allpages' },
  { name: 'recentchanges', labelKey: 'sp_recentchanges' },
  { name: 'categories',    labelKey: 'sp_categories' },
  { name: 'wantedpages',   labelKey: 'sp_wantedpages' },
  { name: 'orphaned',      labelKey: 'sp_orphaned' },
] as const

/** Lowercase + underscores, matching the backend slugify(). */
export function slugify(title: string): string {
  return title.trim().toLowerCase().replace(/[\s_]+/g, '_').replace(/^_+|_+$/g, '')
}

export function pagePath(wikiId: string, ns: string, title: string): string {
  return `/wiki/${wikiId}/page/${ns}/${slugify(title)}`
}

export function editPath(wikiId: string, ns: string, title: string): string {
  return `/wiki/${wikiId}/edit/${ns}/${slugify(title)}`
}
