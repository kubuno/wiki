import { useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { Spinner } from '@ui'
import { wikiApi, pagePath, editPath } from './api'

export default function SpecialView() {
  const { t } = useTranslation('wiki')
  const navigate = useNavigate()
  const { wikiId = '', name = '' } = useParams()

  const title = ({
    allpages: t('sp_allpages'),
    recentchanges: t('sp_recentchanges'),
    categories: t('sp_categories'),
    wantedpages: t('sp_wantedpages'),
    orphaned: t('sp_orphaned'),
  } as Record<string, string>)[name] ?? name

  return (
    <div className="max-w-4xl mx-auto px-6 py-6">
      <h1 className="text-xl font-semibold text-text-primary mb-4">{title}</h1>
      {name === 'allpages' && <AllPages wikiId={wikiId} nav={navigate} />}
      {name === 'orphaned' && <PageList queryKey={['wiki-orphaned', wikiId]} fetcher={() => wikiApi.orphanedPages(wikiId)} wikiId={wikiId} nav={navigate} />}
      {name === 'recentchanges' && <RecentChanges wikiId={wikiId} nav={navigate} />}
      {name === 'wantedpages' && <WantedPages wikiId={wikiId} nav={navigate} />}
      {name === 'categories' && <Categories wikiId={wikiId} nav={navigate} />}
    </div>
  )
}

function Empty() {
  const { t } = useTranslation('wiki')
  return <p className="text-text-tertiary text-sm">{t('no_results')}</p>
}

function AllPages({ wikiId, nav }: { wikiId: string; nav: (to: string) => void }) {
  return <PageList queryKey={['wiki-allpages', wikiId]} fetcher={() => wikiApi.allPages(wikiId)} wikiId={wikiId} nav={nav} />
}

function PageList({ queryKey, fetcher, wikiId, nav }: {
  queryKey: unknown[]; fetcher: () => Promise<import('./api').PageSummary[]>; wikiId: string; nav: (to: string) => void
}) {
  const { data = [], isLoading } = useQuery({ queryKey, queryFn: fetcher })
  if (isLoading) return <Spinner />
  if (data.length === 0) return <Empty />
  return (
    <ul className="divide-y divide-border border border-border rounded-lg overflow-hidden">
      {data.map(p => (
        <li key={p.id} className="px-3 py-2 hover:bg-surface-1">
          <button className="text-primary hover:underline" onClick={() => nav(pagePath(wikiId, p.namespace, p.title))}>
            {p.namespace === 'Main' ? p.title : `${p.namespace}:${p.title}`}
          </button>
        </li>
      ))}
    </ul>
  )
}

function RecentChanges({ wikiId, nav }: { wikiId: string; nav: (to: string) => void }) {
  const { t } = useTranslation('wiki')
  const { data = [], isLoading } = useQuery({ queryKey: ['wiki-rc-full', wikiId], queryFn: () => wikiApi.recentChanges(wikiId, 200) })
  if (isLoading) return <Spinner />
  if (data.length === 0) return <Empty />
  return (
    <ul className="divide-y divide-border border border-border rounded-lg overflow-hidden">
      {data.map(c => (
        <li key={c.id} className="px-3 py-2 text-sm flex items-center gap-2 hover:bg-surface-1">
          <span className="text-text-tertiary text-xs w-36 shrink-0">{new Date(c.created_at).toLocaleString()}</span>
          <button className="text-primary hover:underline truncate" onClick={() => nav(pagePath(wikiId, c.namespace, c.title))}>
            {c.namespace === 'Main' ? c.title : `${c.namespace}:${c.title}`}
          </button>
          <span className="text-text-secondary text-xs">· {t(`change_${c.change_type}`)} · {c.author_name || '—'}</span>
          {c.byte_delta !== 0 && (
            <span className={`text-xs ${c.byte_delta > 0 ? 'text-success' : 'text-danger'}`}>
              {c.byte_delta > 0 ? '+' : ''}{c.byte_delta}
            </span>
          )}
        </li>
      ))}
    </ul>
  )
}

function WantedPages({ wikiId, nav }: { wikiId: string; nav: (to: string) => void }) {
  const { t } = useTranslation('wiki')
  const { data = [], isLoading } = useQuery({ queryKey: ['wiki-wanted', wikiId], queryFn: () => wikiApi.wantedPages(wikiId) })
  if (isLoading) return <Spinner />
  if (data.length === 0) return <Empty />
  return (
    <ul className="divide-y divide-border border border-border rounded-lg overflow-hidden">
      {data.map(p => (
        <li key={`${p.namespace}:${p.slug}`} className="px-3 py-2 text-sm flex items-center gap-2 hover:bg-surface-1">
          <button className="text-danger hover:underline" onClick={() => nav(editPath(wikiId, p.namespace, p.title))}>
            {p.namespace === 'Main' ? p.title : `${p.namespace}:${p.title}`}
          </button>
          <span className="text-text-tertiary text-xs">· {t('refs', { count: p.refs })}</span>
        </li>
      ))}
    </ul>
  )
}

function Categories({ wikiId, nav }: { wikiId: string; nav: (to: string) => void }) {
  const { t } = useTranslation('wiki')
  const { data = [], isLoading } = useQuery({ queryKey: ['wiki-cats', wikiId], queryFn: () => wikiApi.categories(wikiId) })
  if (isLoading) return <Spinner />
  if (data.length === 0) return <Empty />
  return (
    <ul className="divide-y divide-border border border-border rounded-lg overflow-hidden">
      {data.map(c => (
        <li key={c.slug} className="px-3 py-2 text-sm flex items-center gap-2 hover:bg-surface-1">
          <button className="text-primary hover:underline" onClick={() => nav(`/wiki/${wikiId}/category/${c.slug}`)}>{c.title}</button>
          <span className="text-text-tertiary text-xs">· {t('pages_count', { count: c.pages })}</span>
        </li>
      ))}
    </ul>
  )
}
