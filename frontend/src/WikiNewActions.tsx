import { i18n, prompt, navigate } from '@kubuno/sdk'
import type { MenuItem } from '@ui'
import { FilePlus2, BookPlus } from 'lucide-react'
import { editPath } from './api'
import { getActiveWiki } from './nav'

/**
 * Items for the sidebar "New" button (`shell.new-actions` extension point).
 * Built when the menu opens — fresh labels and active wiki, no hooks.
 */
export function newActionItems(): MenuItem[] {
  const t = (key: string) => i18n.t(`wiki:${key}`)

  const newPage = async () => {
    const wikiId = getActiveWiki()
    if (!wikiId) { navigate('/wiki?new=1'); return }
    const title = await prompt({ title: t('new_page'), placeholder: t('page_title'), confirmLabel: t('create') })
    if (!title?.trim()) return
    navigate(editPath(wikiId, 'Main', title.trim()))
  }

  return [
    {
      type: 'action',
      label: t('new_page'),
      icon: <FilePlus2 size={16} />,
      onClick: () => { void newPage() },
    },
    {
      type: 'action',
      label: t('new_wiki'),
      icon: <BookPlus size={16} />,
      onClick: () => navigate('/wiki?new=1'),
    },
  ]
}
