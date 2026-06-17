import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Button, Input, Spinner, ConfirmDialog } from '@ui'
import { useConfirm } from '@kubuno/sdk'
import { Trash2, UserPlus } from 'lucide-react'
import { wikiApi } from './api'

const ROLES = ['admin', 'editor', 'reader']

export default function WikiSettingsPage() {
  const { t } = useTranslation('wiki')
  const navigate = useNavigate()
  const qc = useQueryClient()
  const { wikiId } = useParams()
  const { confirm, confirmState, handleConfirm, handleCancel } = useConfirm()

  const { data: wiki, isLoading } = useQuery({
    queryKey: ['wiki', wikiId],
    queryFn: () => wikiApi.getWiki(wikiId!),
    enabled: !!wikiId,
  })

  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [savedOnce, setSavedOnce] = useState(false)

  useEffect(() => {
    if (wiki && !savedOnce) { setName(wiki.name); setDescription(wiki.description); setSavedOnce(true) }
  }, [wiki, savedOnce])

  if (!wikiId) {
    return (
      <div className="max-w-2xl mx-auto px-6 py-8 text-text-secondary">
        <h1 className="text-xl font-semibold text-text-primary mb-3">{t('wiki')}</h1>
        <Button variant="primary" onClick={() => navigate('/wiki')}>{t('back_to_wikis')}</Button>
      </div>
    )
  }
  if (isLoading || !wiki) return <div className="flex justify-center py-16"><Spinner /></div>

  const isAdmin = wiki.my_role === 'owner' || wiki.my_role === 'admin'

  const saveInfo = async () => {
    await wikiApi.updateWiki(wikiId, { name: name.trim(), description: description.trim() })
    qc.invalidateQueries({ queryKey: ['wiki', wikiId] })
  }

  const doDelete = async () => {
    if (await confirm({ title: t('delete_wiki'), message: t('confirm_delete_wiki'), confirmLabel: t('delete_wiki'), variant: 'danger' })) {
      await wikiApi.deleteWiki(wikiId)
      qc.invalidateQueries({ queryKey: ['wikis'] })
      navigate('/wiki')
    }
  }

  return (
    <div className="max-w-2xl mx-auto px-6 py-8">
      <h1 className="text-xl font-semibold text-text-primary mb-5">{t('wiki_settings')}</h1>

      {isAdmin && (
        <section className="mb-8">
          <Input label={t('wiki_name')} value={name} onChange={(e) => setName(e.target.value)} />
          <div className="mt-3">
            <label className="block text-xs font-medium text-text-secondary mb-1">{t('description')}</label>
            <textarea value={description} onChange={(e) => setDescription(e.target.value)} rows={3}
              className="w-full rounded-md border border-border px-2.5 py-1.5 text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary/30" />
          </div>
          <Button variant="primary" className="mt-3" onClick={saveInfo}>{t('save_changes')}</Button>
        </section>
      )}

      {wiki.is_shared && isAdmin && <MembersSection wikiId={wikiId} ownerRole={wiki.my_role} />}

      {wiki.my_role === 'owner' && (
        <section className="mt-10 border border-danger/30 rounded-lg p-4">
          <h2 className="text-sm font-semibold text-danger mb-2">{t('danger_zone')}</h2>
          <Button variant="ghost" className="text-danger" onClick={doDelete}><Trash2 size={16} /> {t('delete_wiki')}</Button>
        </section>
      )}

      {confirmState && <ConfirmDialog {...confirmState} onConfirm={handleConfirm} onCancel={handleCancel} />}
    </div>
  )
}

function MembersSection({ wikiId, ownerRole }: { wikiId: string; ownerRole: string }) {
  const { t } = useTranslation('wiki')
  const qc = useQueryClient()
  const { data: members = [] } = useQuery({ queryKey: ['wiki-members', wikiId], queryFn: () => wikiApi.listMembers(wikiId) })
  const [email, setEmail] = useState('')
  const [role, setRole] = useState('editor')
  const [error, setError] = useState('')
  const refresh = () => qc.invalidateQueries({ queryKey: ['wiki-members', wikiId] })

  const add = async () => {
    if (!email.trim()) return
    setError('')
    try { await wikiApi.addMember(wikiId, { email: email.trim(), role }); setEmail(''); refresh() }
    catch { setError(t('no_results')) }
  }

  return (
    <section className="mb-8">
      <h2 className="text-sm font-semibold text-text-secondary mb-3">{t('manage_members')}</h2>

      <div className="flex flex-wrap items-end gap-2 mb-3">
        <div className="flex-1 min-w-[180px]">
          <Input label={t('member_email')} value={email} onChange={(e) => setEmail(e.target.value)} placeholder="user@example.com" />
        </div>
        <select value={role} onChange={(e) => setRole(e.target.value)} className="rounded-md border border-border px-2 py-1.5 text-sm">
          {ROLES.map(r => <option key={r} value={r}>{t(`role_${r}`)}</option>)}
        </select>
        <Button variant="primary" onClick={add}><UserPlus size={16} /> {t('add_member')}</Button>
      </div>
      {error && <p className="text-xs text-danger mb-2">{error}</p>}

      <ul className="divide-y divide-border border border-border rounded-lg overflow-hidden">
        {members.map(m => (
          <li key={m.user_id} className="px-3 py-2 flex items-center gap-2 text-sm">
            <div className="flex-1 min-w-0">
              <div className="text-text-primary truncate">{m.display_name || m.email}</div>
              <div className="text-text-tertiary text-xs truncate">{m.email}</div>
            </div>
            {m.role === 'owner' ? (
              <span className="text-xs text-text-secondary">{t('role_owner')}</span>
            ) : (
              <>
                <select
                  value={m.role}
                  onChange={async (e) => { await wikiApi.updateMember(wikiId, m.user_id, e.target.value); refresh() }}
                  className="rounded-md border border-border px-2 py-1 text-xs"
                  disabled={ownerRole !== 'owner' && ownerRole !== 'admin'}
                >
                  {ROLES.map(r => <option key={r} value={r}>{t(`role_${r}`)}</option>)}
                </select>
                <button className="text-danger p-1 hover:bg-surface-1 rounded"
                  onClick={async () => { await wikiApi.removeMember(wikiId, m.user_id); refresh() }}>
                  <Trash2 size={15} />
                </button>
              </>
            )}
          </li>
        ))}
      </ul>
    </section>
  )
}
