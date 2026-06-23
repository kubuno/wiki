import { useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { MenuDropdown, ConfirmDialog, Button, Spinner, type MenuItem } from '@ui'
import { useConfirm, prompt } from '@kubuno/sdk'
import { MoreVertical, Pencil, History as HistoryIcon, MessageSquare, BookOpen } from 'lucide-react'
import { wikiApi, pagePath, editPath, slugify } from './api'

export default function PageView() {
  const { t } = useTranslation('wiki')
  const navigate = useNavigate()
  const qc = useQueryClient()
  const { wikiId = '', ns = 'Main', title = '' } = useParams()
  const { confirm, confirmState, handleConfirm, handleCancel } = useConfirm()
  const [menu, setMenu] = useState<{ top: number; left: number } | null>(null)
  const [searchParams] = useSearchParams()
  const historyMode = searchParams.get('history') === '1'

  const { data: page, isLoading } = useQuery({
    queryKey: ['wiki-page', wikiId, ns, title],
    queryFn: () => wikiApi.getPage(wikiId, ns, title),
  })

  const { data: revisions = [] } = useQuery({
    queryKey: ['wiki-history', wikiId, page?.id],
    queryFn: () => wikiApi.history(wikiId, page!.id!),
    enabled: historyMode && !!page?.id,
  })

  const { data: backlinks = [] } = useQuery({
    queryKey: ['wiki-backlinks', wikiId, page?.id],
    queryFn: () => wikiApi.backlinks(wikiId, page!.id!),
    enabled: !!page?.exists && !!page?.id,
  })

  if (isLoading || !page) return <div className="flex justify-center py-16"><Spinner /></div>

  // Intercept internal wiki link clicks → SPA navigation.
  const onContentClick = (e: React.MouseEvent) => {
    const a = (e.target as HTMLElement).closest('a')
    if (!a) return
    const lns = a.getAttribute('data-ns')
    const lt = a.getAttribute('data-title')
    if (lns && lt) { e.preventDefault(); navigate(pagePath(wikiId, lns, lt)) }
  }

  const doMove = async () => {
    if (!page.id) return
    const target = await prompt({ title: t('move_page'), placeholder: t('move_to'), defaultValue: page.prefixed_title, confirmLabel: t('save') })
    if (!target?.trim()) return
    const moved = await wikiApi.movePage(wikiId, page.id, target.trim())
    navigate(pagePath(wikiId, moved.namespace, moved.title))
  }

  const doDelete = async () => {
    if (!page.id) return
    if (await confirm({ title: t('delete_page'), message: t('confirm_delete_page'), confirmLabel: t('delete_page'), variant: 'danger' })) {
      await wikiApi.deletePage(wikiId, page.id)
      qc.invalidateQueries({ queryKey: ['wiki-page', wikiId, ns, title] })
      navigate(`/wiki/${wikiId}`)
    }
  }

  const actionItems: MenuItem[] = [
    { type: 'action', label: t('move_page'), onClick: () => { setMenu(null); doMove() } },
    { type: 'action', label: t('delete_page'), danger: true, onClick: () => { setMenu(null); doDelete() } },
  ]

  const tab = (label: string, icon: React.ReactNode, onClick: () => void, active = false) => (
    <button
      onClick={onClick}
      className={`flex items-center gap-1.5 px-3 py-1.5 text-sm border-b-2 -mb-px ${active
        ? 'border-primary text-primary font-medium'
        : 'border-transparent text-text-secondary hover:text-text-primary'}`}
    >
      {icon}{label}
    </button>
  )

  return (
    <div className="max-w-5xl mx-auto px-6 py-5">
      {/* Title + tabs */}
      <div className="border-b border-border mb-4">
        <div className="flex items-start justify-between">
          <h1 className="text-2xl font-serif text-text-primary pb-2">{page.prefixed_title}</h1>
          {page.can_edit && page.exists && (
            <button className="p-1.5 rounded hover:bg-surface-1 text-text-secondary no-print"
              onClick={(e) => setMenu({ top: e.clientY, left: e.clientX })}>
              <MoreVertical size={18} />
            </button>
          )}
        </div>
        <div className="flex gap-1 no-print">
          {tab(t('read'), <BookOpen size={15} />, () => {}, true)}
          {page.can_edit && tab(t('edit'), <Pencil size={15} />, () => navigate(editPath(wikiId, ns, title)))}
          {page.exists && tab(t('history'), <HistoryIcon size={15} />, () => navigate(`${pagePath(wikiId, ns, title)}?history=1`))}
          {page.talk_namespace && tab(t('talk'), <MessageSquare size={15} />, () => navigate(pagePath(wikiId, page.talk_namespace!, title)))}
        </div>
      </div>

      {page.redirect && (
        <div className="text-sm text-text-secondary mb-3">
          {t('redirect_from')}{' '}
          <button className="text-primary hover:underline" onClick={() => navigate(pagePath(wikiId, 'Main', page.redirect!))}>
            {page.redirect}
          </button>
        </div>
      )}

      {historyMode && page.exists ? (
        <div>
          <h2 className="text-sm font-semibold text-text-secondary mb-2">{t('history')}</h2>
          <ul className="divide-y divide-border border border-border rounded-lg overflow-hidden">
            {revisions.map(r => (
              <li key={r.rev_id} className="px-3 py-2 text-sm flex items-center gap-2">
                <span className="text-text-primary">{new Date(r.ts).toLocaleString()}</span>
                <span className="text-text-secondary">· {r.author_name || '—'}</span>
                {r.minor && <span className="text-xs text-text-tertiary">· m</span>}
                {r.comment && <span className="text-text-tertiary italic truncate">· {r.comment}</span>}
                <span className="ml-auto text-xs text-text-tertiary">{r.size} B</span>
              </li>
            ))}
          </ul>
          <Button variant="ghost" className="mt-3" onClick={() => navigate(pagePath(wikiId, ns, title))}>{t('read')}</Button>
        </div>
      ) : !page.exists ? (
        <div className="py-10 text-center">
          <p className="text-text-secondary mb-4">{t('does_not_exist')}</p>
          {page.can_edit && (
            <Button variant="primary" onClick={() => navigate(editPath(wikiId, ns, title))}>{t('create_this_page')}</Button>
          )}
        </div>
      ) : (
        <div className="flex gap-6">
          {page.toc && page.toc.length > 1 && (
            <nav className="hidden lg:block w-56 shrink-0 sticky top-4 self-start text-sm no-print">
              <div className="font-semibold text-text-secondary mb-1">{t('table_of_contents')}</div>
              <ul className="space-y-0.5">
                {page.toc.map((e, i) => (
                  <li key={i} style={{ paddingLeft: (e.level - 2) * 12 }}>
                    <a href={`#${e.id}`} className="text-primary hover:underline">{e.text}</a>
                  </li>
                ))}
              </ul>
            </nav>
          )}
          <div className="flex-1 min-w-0">
            <article
              className="wiki-content"
              onClick={onContentClick}
              dangerouslySetInnerHTML={{ __html: page.html }}
            />

            {page.categories.length > 0 && (
              <div className="mt-8 pt-3 border-t border-border flex flex-wrap items-center gap-2 text-sm">
                <span className="text-text-secondary">{t('in_categories')}:</span>
                {page.categories.map(c => (
                  <button key={c.slug} className="text-primary hover:underline"
                    onClick={() => navigate(`/wiki/${wikiId}/category/${c.slug}`)}>
                    {c.title}
                  </button>
                ))}
              </div>
            )}

            <details className="mt-6 text-sm">
              <summary className="cursor-pointer text-text-secondary">{t('what_links_here')} ({backlinks.length})</summary>
              {backlinks.length === 0 ? (
                <p className="text-text-tertiary mt-2">{t('no_backlinks')}</p>
              ) : (
                <ul className="mt-2 space-y-1">
                  {backlinks.map(b => (
                    <li key={b.id}>
                      <button className="text-primary hover:underline"
                        onClick={() => navigate(pagePath(wikiId, b.namespace, b.title))}>
                        {b.namespace === 'Main' ? b.title : `${b.namespace}:${b.title}`}
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </details>
          </div>
        </div>
      )}

      {menu && <MenuDropdown items={actionItems} pos={menu} onClose={() => setMenu(null)} />}
      {confirmState && <ConfirmDialog {...confirmState} onConfirm={handleConfirm} onCancel={handleCancel} />}
    </div>
  )
}
