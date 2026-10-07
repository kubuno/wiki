import { useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { Button, Spinner } from '@ui'
import { BookMarked, FileText, History, Plus } from 'lucide-react'
import { wikiApi, pagePath, editPath } from './api'

const HOME_TITLE = 'Accueil'

export default function WikiHome() {
  const { t } = useTranslation('wiki')
  const navigate = useNavigate()
  const { wikiId = '' } = useParams()

  const { data: wiki, isLoading } = useQuery({ queryKey: ['wiki', wikiId], queryFn: () => wikiApi.getWiki(wikiId) })
  const { data: changes = [] } = useQuery({ queryKey: ['wiki-rc', wikiId], queryFn: () => wikiApi.recentChanges(wikiId, 12) })

  if (isLoading || !wiki) return <div className="flex justify-center py-16"><Spinner /></div>

  const canEdit = wiki.my_role !== 'reader' && wiki.my_role !== 'none'

  return (
    <div className="max-w-4xl mx-auto px-6 py-6">
      <div className="flex items-start gap-3 mb-2">
        <BookMarked size={28} className="text-[#0f766e] mt-1" />
        <div className="flex-1">
          <h1 className="text-2xl font-semibold text-text-primary">{wiki.name}</h1>
          {wiki.description && <p className="text-text-secondary mt-1">{wiki.description}</p>}
          <p className="text-xs text-text-tertiary mt-1">{t('pages_count', { count: wiki.page_count })}</p>
        </div>
      </div>

      <div className="flex gap-2 my-4">
        <Button variant="primary" onClick={() => navigate(pagePath(wikiId, 'Main', HOME_TITLE))}>
          <FileText size={16} /> {HOME_TITLE}
        </Button>
        <Button variant="ghost" onClick={() => navigate(`/wiki/${wikiId}/special/allpages`)}>
          {t('all_pages')}
        </Button>
        {canEdit && wiki.page_count === 0 && (
          <Button variant="ghost" onClick={() => navigate(editPath(wikiId, 'Main', HOME_TITLE))}>
            <Plus size={16} /> {t('start_page')}
          </Button>
        )}
      </div>

      <h2 className="flex items-center gap-2 text-sm font-semibold text-text-secondary mt-6 mb-2">
        <History size={16} /> {t('recent_changes')}
      </h2>
      {changes.length === 0 ? (
        <p className="text-sm text-text-tertiary">{t('no_pages')}</p>
      ) : (
        <ul className="divide-y divide-border border border-border rounded-lg overflow-hidden">
          {changes.map(c => (
            <li key={c.id} className="px-3 py-2 text-sm flex items-center gap-2 hover:bg-surface-1">
              <button
                className="text-primary hover:underline truncate"
                onClick={() => navigate(pagePath(wikiId, c.namespace, c.title))}
              >
                {c.namespace === 'Main' ? c.title : `${c.namespace}:${c.title}`}
              </button>
              <span className="text-text-tertiary text-xs">· {t(`change_${c.change_type}`)} · {c.author_name || '—'}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
