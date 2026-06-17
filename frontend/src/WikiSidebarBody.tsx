import { useEffect } from 'react'
import { useNavigate, useParams, useLocation } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { Library, Home, FileStack, History, Tags, HelpCircle, Unlink, Users, Settings } from 'lucide-react'
import { SidebarNavItem } from '@kubuno/sdk'
import { wikiApi } from './api'
import { setNav, setActiveWiki } from './nav'

export default function WikiSidebarBody({ collapsed = false }: { collapsed?: boolean }) {
  const { t } = useTranslation('wiki')
  const navigate = useNavigate()
  const { pathname } = useLocation()
  const params = useParams()
  const wikiId = params.wikiId ?? null

  useEffect(() => setNav(navigate), [navigate])
  useEffect(() => { setActiveWiki(wikiId) }, [wikiId])

  const { data: wiki } = useQuery({
    queryKey: ['wiki', wikiId],
    queryFn: () => wikiApi.getWiki(wikiId!),
    enabled: !!wikiId,
  })

  const item = (label: string, icon: React.ReactNode, to: string, active: boolean) => (
    <SidebarNavItem label={label} icon={icon} collapsed={collapsed} active={active} onClick={() => navigate(to)} />
  )

  return (
    <div className="flex flex-col gap-0.5 px-2 py-2">
      {item(t('back_to_wikis'), <Library size={18} />, '/wiki', pathname === '/wiki')}

      {wikiId && wiki && (
        <>
          {!collapsed && (
            <div className="px-2 mt-3 mb-1 text-[11px] font-semibold uppercase tracking-wide text-text-tertiary truncate">
              {wiki.name}
            </div>
          )}
          {item(t('home'), <Home size={18} />, `/wiki/${wikiId}`, pathname === `/wiki/${wikiId}`)}
          {item(t('all_pages'), <FileStack size={18} />, `/wiki/${wikiId}/special/allpages`, pathname.endsWith('/special/allpages'))}
          {item(t('recent_changes'), <History size={18} />, `/wiki/${wikiId}/special/recentchanges`, pathname.endsWith('/special/recentchanges'))}
          {item(t('categories'), <Tags size={18} />, `/wiki/${wikiId}/special/categories`, pathname.endsWith('/special/categories'))}
          {item(t('wanted_pages'), <HelpCircle size={18} />, `/wiki/${wikiId}/special/wantedpages`, pathname.endsWith('/special/wantedpages'))}
          {item(t('orphaned'), <Unlink size={18} />, `/wiki/${wikiId}/special/orphaned`, pathname.endsWith('/special/orphaned'))}

          {wiki.is_shared && (wiki.my_role === 'owner' || wiki.my_role === 'admin') &&
            item(t('members'), <Users size={18} />, `/wiki/${wikiId}/settings`, false)}
          {(wiki.my_role === 'owner' || wiki.my_role === 'admin') &&
            item(t('settings'), <Settings size={18} />, `/wiki/${wikiId}/settings`, pathname.endsWith('/settings'))}
        </>
      )}
    </div>
  )
}
