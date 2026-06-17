import * as DropdownMenu from '@radix-ui/react-dropdown-menu'
import { FilePlus2, BookPlus } from 'lucide-react'
import { useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { prompt } from '@kubuno/sdk'
import { editPath } from './api'

const ITEM_CLASS =
  'flex items-center gap-3 w-full px-3 py-2 text-sm text-text-primary ' +
  'hover:bg-surface-1 cursor-pointer outline-none'

export default function WikiNewActions() {
  const navigate = useNavigate()
  const { t } = useTranslation('wiki')
  const params = useParams()
  const wikiId = params.wikiId ?? null

  const newPage = async () => {
    if (!wikiId) { navigate('/wiki?new=1'); return }
    const title = await prompt({ title: t('new_page'), placeholder: t('page_title'), confirmLabel: t('create') })
    if (!title?.trim()) return
    navigate(editPath(wikiId, 'Main', title.trim()))
  }

  return (
    <>
      <DropdownMenu.Item onSelect={newPage} className={ITEM_CLASS}>
        <FilePlus2 size={16} className="text-text-secondary" />
        {t('new_page')}
      </DropdownMenu.Item>
      <DropdownMenu.Item onSelect={() => navigate('/wiki?new=1')} className={ITEM_CLASS}>
        <BookPlus size={16} className="text-text-secondary" />
        {t('new_wiki')}
      </DropdownMenu.Item>
    </>
  )
}
