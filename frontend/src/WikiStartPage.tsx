import { useEffect, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { format } from 'date-fns'
import { StartPage, FloatingWindow, Button, Input, Spinner } from '@ui'
import type { StartPageRecentItem, StartPageTab } from '@ui'
import { ModuleFileBrowser, type FileItem } from '@kubuno/drive'
import { getDateLocale } from '@kubuno/sdk'
import { BookMarked, Plus, Users, Lock, FileText } from 'lucide-react'
import { wikiApi, pagePath } from './api'

export default function WikiStartPage() {
  const { t, i18n } = useTranslation('wiki')
  const navigate = useNavigate()
  const [params, setParams] = useSearchParams()
  const [creating, setCreating] = useState(false)

  useEffect(() => {
    if (params.get('new') === '1') { setCreating(true); params.delete('new'); setParams(params, { replace: true }) }
  }, [params, setParams])

  // Recents launcher: recently edited pages across every accessible wiki.
  const { data: recents = [] } = useQuery({ queryKey: ['wiki-recent-pages'], queryFn: () => wikiApi.recentPages(12) })

  // Open a .kbwik file from the Browse tab → its page.
  const handleOpenFile = (file: FileItem): boolean => {
    wikiApi.openByFile(file.id).then(p => navigate(pagePath(p.wiki_id, p.namespace, p.title))).catch(() => {})
    return true
  }

  const recentItems: StartPageRecentItem[] = recents.map(p => ({
    id:       `${p.wiki_id}:${p.namespace}:${p.slug}`,
    name:     p.namespace === 'Main' ? p.title : `${p.namespace}:${p.title}`,
    subtitle: p.current_rev_at ? format(new Date(p.current_rev_at), 'd MMM', { locale: getDateLocale(i18n.language) }) : undefined,
    icon:     <FileText size={18} className="text-text-tertiary" strokeWidth={1.5} />,
    onClick:  () => navigate(pagePath(p.wiki_id, p.namespace, p.title)),
  }))

  const newWikiButton = (
    <Button size="sm" icon={<Plus size={15} />} onClick={() => setCreating(true)}>{t('new_wiki')}</Button>
  )

  // Onglet « Parcourir » par défaut (navigateur de fichiers plein cadre), comme
  // le sous-module Documents ; « Mes wikis » en second (gestion des espaces).
  const tabs: StartPageTab[] = [
    {
      id: 'browse',
      label: t('browse'),
      content: (
        <ModuleFileBrowser
          folderPathPrefix="Wiki"
          title="Wiki"
          fileTypeModuleId="wiki"
          onOpenFile={handleOpenFile}
          toolbarContent={newWikiButton}
          emptyState={
            <div className="flex flex-col items-center justify-center py-24 text-center">
              <BookMarked size={48} className="text-text-tertiary mb-4 opacity-30" />
              <p className="text-text-secondary font-medium mb-1">{t('no_wikis')}</p>
              <button onClick={() => setCreating(true)} className="text-sm text-primary hover:underline mt-1">{t('create_wiki')}</button>
            </div>
          }
        />
      ),
    },
    {
      id: 'wikis',
      label: t('my_wikis'),
      content: <WikisGrid onCreate={() => setCreating(true)} />,
    },
  ]

  return (
    <>
      <StartPage
        recentTitle={t('recent')}
        recentIcon={<FileText size={15} />}
        recentItems={recentItems}
        recentEmpty={
          <div className="flex flex-col items-center gap-2">
            <FileText size={32} className="text-text-tertiary opacity-30" strokeWidth={1.5} />
            <p className="text-text-tertiary text-xs">{t('no_recent_pages')}</p>
          </div>
        }
        tabs={tabs}
        defaultTab="browse"
      />
      {creating && (
        <CreateWikiDialog
          onClose={() => setCreating(false)}
          onCreated={(id) => navigate(`/wiki/${id}`)}
        />
      )}
    </>
  )
}

// ── "My wikis" grid (the wiki spaces) ────────────────────────────────────────

function WikisGrid({ onCreate }: { onCreate: () => void }) {
  const { t } = useTranslation('wiki')
  const navigate = useNavigate()
  const { data: wikis = [], isLoading } = useQuery({ queryKey: ['wikis'], queryFn: wikiApi.listWikis })

  return (
    <div className="h-full overflow-y-auto px-6 py-5">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-base font-semibold text-text-primary">{t('my_wikis')}</h2>
        <Button variant="primary" onClick={onCreate}><Plus size={16} /> {t('create_wiki')}</Button>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-16"><Spinner /></div>
      ) : wikis.length === 0 ? (
        <div className="text-center py-16 text-text-secondary">{t('no_wikis')}</div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {wikis.map(w => (
            <button
              key={w.id}
              onClick={() => navigate(`/wiki/${w.id}`)}
              className="text-left bg-surface-0 border border-border rounded-xl p-4 hover:shadow-md transition-shadow"
            >
              <div className="flex items-center gap-2 mb-2">
                <BookMarked size={20} className="text-[#0f766e]" />
                <span className="font-semibold text-text-primary truncate">{w.name}</span>
              </div>
              <p className="text-sm text-text-secondary line-clamp-2 min-h-[2.5rem]">{w.description || '—'}</p>
              <div className="flex items-center gap-3 mt-3 text-xs text-text-tertiary">
                <span>{t('pages_count', { count: w.page_count })}</span>
                <span className="inline-flex items-center gap-1">
                  {w.is_shared ? <Users size={13} /> : <Lock size={13} />}
                  {w.is_shared ? t('shared') : t('personal')}
                </span>
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

// ── Create wiki dialog ───────────────────────────────────────────────────────

export function CreateWikiDialog({ onClose, onCreated }: { onClose: () => void; onCreated: (id: string) => void }) {
  const { t } = useTranslation('wiki')
  const qc = useQueryClient()
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [isShared, setIsShared] = useState(false)
  const [busy, setBusy] = useState(false)

  const submit = async () => {
    if (!name.trim()) return
    setBusy(true)
    try {
      const wiki = await wikiApi.createWiki({ name: name.trim(), description: description.trim(), is_shared: isShared })
      qc.invalidateQueries({ queryKey: ['wikis'] })
      onCreated(wiki.id)
    } finally { setBusy(false) }
  }

  return (
    <FloatingWindow title={t('create_wiki')} icon={<BookMarked size={18} />} onClose={onClose} defaultWidth={480} defaultHeight={360}>
      <div className="flex flex-col gap-3 p-4 h-full">
        <Input label={t('wiki_name')} value={name} autoFocus onChange={(e) => setName(e.target.value)} placeholder={t('wiki_name')} />
        <div>
          <label className="block text-xs font-medium text-text-secondary mb-1">{t('description')}</label>
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={3}
            className="w-full rounded-md border border-border px-2.5 py-1.5 text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary/30"
          />
        </div>
        <label className="flex items-start gap-2 text-sm text-text-primary cursor-pointer">
          <input type="checkbox" checked={isShared} onChange={(e) => setIsShared(e.target.checked)} className="mt-0.5" />
          <span>
            {t('shared_wiki')}
            <span className="block text-xs text-text-tertiary">{t('shared_wiki_hint')}</span>
          </span>
        </label>
        <div className="flex justify-end gap-2 mt-auto">
          <Button variant="ghost" onClick={onClose}>{t('cancel')}</Button>
          <Button variant="primary" loading={busy} disabled={!name.trim()} onClick={submit}>{t('create')}</Button>
        </div>
      </div>
    </FloatingWindow>
  )
}
