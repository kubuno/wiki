import { useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { Spinner } from '@ui'
import { useWikiStore } from './store'
import { wikiApi, pagePath } from './api'

export default function SearchView() {
  const { t } = useTranslation('wiki')
  const navigate = useNavigate()
  const { wikiId = '' } = useParams()
  const q = useWikiStore(s => s.searchQuery)

  const { data = [], isLoading } = useQuery({
    queryKey: ['wiki-search', wikiId, q],
    queryFn: () => wikiApi.search(wikiId, q),
    enabled: q.trim().length > 0,
  })

  return (
    <div className="max-w-3xl mx-auto px-6 py-6">
      <h1 className="text-xl font-semibold text-text-primary mb-1">{t('search_results')}</h1>
      <p className="text-sm text-text-secondary mb-4">{t('search_for', { q })}</p>
      {isLoading ? <Spinner /> : data.length === 0 ? (
        <p className="text-text-tertiary text-sm">{t('no_search_results')}</p>
      ) : (
        <ul className="space-y-3">
          {data.map(h => (
            <li key={h.id}>
              <button className="text-primary hover:underline font-medium" onClick={() => navigate(pagePath(wikiId, h.namespace, h.title))}>
                {h.namespace === 'Main' ? h.title : `${h.namespace}:${h.title}`}
              </button>
              <p className="text-sm text-text-secondary line-clamp-2">{h.preview}</p>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
