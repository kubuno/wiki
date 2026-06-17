import { useEffect, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { Button, Input, Spinner } from '@ui'
import { Bold, Italic, Heading, Link2, Braces, Tag, List } from 'lucide-react'
import { wikiApi, pagePath, type PreviewResponse } from './api'

const NAMESPACES = ['Main', 'Talk', 'User', 'Wiki', 'Template', 'Category', 'Help', 'File']

export default function PageEditor() {
  const { t } = useTranslation('wiki')
  const navigate = useNavigate()
  const { wikiId = '', ns: routeNs = 'Main', title: routeTitle = '' } = useParams()
  const taRef = useRef<HTMLTextAreaElement>(null)

  const { data: page, isLoading } = useQuery({
    queryKey: ['wiki-page', wikiId, routeNs, routeTitle],
    queryFn: () => wikiApi.getPage(wikiId, routeNs, routeTitle),
  })

  const [namespace, setNamespace] = useState(routeNs)
  const [title, setTitle] = useState('')
  const [content, setContent] = useState('')
  const [summary, setSummary] = useState('')
  const [minor, setMinor] = useState(false)
  const [busy, setBusy] = useState(false)
  const [preview, setPreview] = useState<PreviewResponse | null>(null)
  const [ready, setReady] = useState(false)

  useEffect(() => {
    if (page && !ready) {
      setNamespace(page.namespace)
      setTitle(page.title)
      setContent(page.source)
      setReady(true)
    }
  }, [page, ready])

  // Debounced live preview.
  useEffect(() => {
    if (!ready) return
    const handle = setTimeout(() => {
      wikiApi.previewPage(wikiId, { namespace, title, content }).then(setPreview).catch(() => {})
    }, 600)
    return () => clearTimeout(handle)
  }, [content, namespace, title, wikiId, ready])

  if (isLoading || !page) return <div className="flex justify-center py-16"><Spinner /></div>
  if (!page.can_edit) return <div className="p-8 text-text-secondary">403</div>

  const insert = (before: string, after = '') => {
    const ta = taRef.current
    if (!ta) return
    const start = ta.selectionStart, end = ta.selectionEnd
    const sel = content.slice(start, end)
    const next = content.slice(0, start) + before + sel + after + content.slice(end)
    setContent(next)
    requestAnimationFrame(() => {
      ta.focus()
      ta.selectionStart = start + before.length
      ta.selectionEnd = start + before.length + sel.length
    })
  }

  const save = async () => {
    if (!title.trim() || !content.trim()) return
    setBusy(true)
    try {
      const saved = await wikiApi.savePage(wikiId, { namespace, title: title.trim(), content, comment: summary, minor })
      navigate(pagePath(wikiId, saved.namespace, saved.title))
    } finally { setBusy(false) }
  }

  const toolBtn = (icon: React.ReactNode, fn: () => void, label: string) => (
    <button type="button" title={label} onClick={fn} className="p-1.5 rounded hover:bg-surface-2 text-text-secondary">{icon}</button>
  )

  return (
    <div className="max-w-6xl mx-auto px-6 py-5">
      <h1 className="text-lg font-semibold text-text-primary mb-3">
        {page.exists ? t('editing', { title: page.prefixed_title }) : t('creating', { title: title || page.title })}
      </h1>

      <div className="flex flex-wrap items-end gap-3 mb-3">
        <div>
          <label className="block text-xs font-medium text-text-secondary mb-1">{t('namespace')}</label>
          <select
            value={namespace}
            onChange={(e) => setNamespace(e.target.value)}
            disabled={page.exists}
            className="rounded-md border border-border px-2 py-1.5 text-sm disabled:opacity-60"
          >
            {NAMESPACES.map(n => <option key={n} value={n}>{n}</option>)}
          </select>
        </div>
        <div className="flex-1 min-w-[200px]">
          <Input label={t('page_title')} value={title} disabled={page.exists} onChange={(e) => setTitle(e.target.value)} />
        </div>
      </div>

      <div className="flex items-center gap-1 border border-border rounded-t-md bg-surface-1 px-2 py-1">
        {toolBtn(<Bold size={16} />, () => insert('**', '**'), 'Bold')}
        {toolBtn(<Italic size={16} />, () => insert('*', '*'), 'Italic')}
        {toolBtn(<Heading size={16} />, () => insert('\n## ', ''), 'Heading')}
        {toolBtn(<List size={16} />, () => insert('\n- ', ''), 'List')}
        {toolBtn(<Link2 size={16} />, () => insert('[[', ']]'), 'Wiki link')}
        {toolBtn(<Braces size={16} />, () => insert('{{', '}}'), 'Template')}
        {toolBtn(<Tag size={16} />, () => insert('[[Category:', ']]'), 'Category')}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-0 lg:gap-4">
        <textarea
          ref={taRef}
          value={content}
          onChange={(e) => setContent(e.target.value)}
          className="w-full h-[55vh] border border-border rounded-b-md lg:rounded-md px-3 py-2 font-mono text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary/30"
          placeholder={t('content')}
        />
        <div className="hidden lg:block border border-border rounded-md p-4 h-[55vh] overflow-auto bg-surface-0">
          <div className="text-xs uppercase tracking-wide text-text-tertiary mb-2">{t('live_preview')}</div>
          <article className="wiki-content" dangerouslySetInnerHTML={{ __html: preview?.html ?? '' }} />
        </div>
      </div>

      <p className="text-xs text-text-tertiary mt-2">{t('syntax_help')}</p>

      <div className="flex flex-wrap items-center gap-3 mt-3">
        <div className="flex-1 min-w-[200px]">
          <Input label={t('summary')} value={summary} onChange={(e) => setSummary(e.target.value)} />
        </div>
        <label className="flex items-center gap-1.5 text-sm text-text-secondary cursor-pointer mt-4">
          <input type="checkbox" checked={minor} onChange={(e) => setMinor(e.target.checked)} /> {t('minor_edit')}
        </label>
        <div className="flex gap-2 mt-4">
          <Button variant="ghost" onClick={() => navigate(pagePath(wikiId, routeNs, routeTitle))}>{t('cancel')}</Button>
          <Button variant="primary" loading={busy} disabled={!title.trim() || !content.trim()} onClick={save}>
            {busy ? t('saving') : t('save_page')}
          </Button>
        </div>
      </div>
    </div>
  )
}
