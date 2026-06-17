import { useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { Spinner } from '@ui'
import { Tags } from 'lucide-react'
import { wikiApi, pagePath } from './api'

export default function CategoryView() {
  const { t } = useTranslation('wiki')
  const navigate = useNavigate()
  const { wikiId = '', slug = '' } = useParams()

  const { data = [], isLoading } = useQuery({
    queryKey: ['wiki-category', wikiId, slug],
    queryFn: () => wikiApi.categoryMembers(wikiId, slug),
  })

  const label = slug.replace(/_/g, ' ').replace(/^./, c => c.toUpperCase())

  return (
    <div className="max-w-4xl mx-auto px-6 py-6">
      <h1 className="flex items-center gap-2 text-xl font-semibold text-text-primary mb-4">
        <Tags size={22} className="text-[#0f766e]" /> {label}
      </h1>
      {isLoading ? <Spinner /> : data.length === 0 ? (
        <p className="text-text-tertiary text-sm">{t('empty_category')}</p>
      ) : (
        <ul className="divide-y divide-border border border-border rounded-lg overflow-hidden">
          {data.map(p => (
            <li key={p.id} className="px-3 py-2 hover:bg-surface-1">
              <button className="text-primary hover:underline" onClick={() => navigate(pagePath(wikiId, p.namespace, p.title))}>
                {p.namespace === 'Main' ? p.title : `${p.namespace}:${p.title}`}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
