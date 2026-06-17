/**
 * Wiki module bundle — loaded at runtime by the host. Shared specifiers
 * (react, zustand, i18next, @ui, @kubuno/sdk…) are resolved by the host import
 * map; the host imports this file and calls `register()`. `sdkVersion` lets the
 * host reject a contract mismatch.
 */
import { lazy } from 'react'
import {
  RouteRegistry,
  WaffleAppRegistry,
  FileTypeRegistry,
  FaviconRegistry,
  useSidebarStore,
  useSearchStore,
  SDK_VERSION,
} from '@kubuno/sdk'
import './index.css'
import './i18n'
import { useWikiStore } from './store'
import { goTo, getActiveWiki } from './nav'
import WikiLogo from './WikiLogo'
import WikiNewActions from './WikiNewActions'
import WikiSidebarBody from './WikiSidebarBody'

export const sdkVersion = SDK_VERSION

export function register() {
  FaviconRegistry.register('wiki', '/wiki-logo.svg')

  WaffleAppRegistry.register('wiki', 'Wiki', [
    { id: 'wiki', label: 'Wiki', Icon: WikiLogo, path: '/wiki' },
  ])

  // Kubuno file type produced by the wiki (.kbwik) — filter + icon + open.
  FileTypeRegistry.register({
    moduleId: 'wiki', label: 'Wiki', icon: 'BookMarked',
    mimeTypes: ['application/vnd.kubuno.wiki+json'],
    extensions: ['kbwik'],
    open: (f, nav) => {
      import('./api').then(({ wikiApi, slugify }) =>
        wikiApi.openByFile(f.id)
          .then(p => nav(`/wiki/${p.wiki_id}/page/${p.namespace}/${slugify(p.title)}`))
          .catch(() => {}))
    },
  })

  useSidebarStore.getState().register({
    moduleId:    'wiki',
    routePrefix: '/wiki',
    newButtonLabelKey: 'wiki:new_page',
    NewActions:  WikiNewActions,
    SidebarBody: WikiSidebarBody,
    collapsedBody: true,
  })

  useSearchStore.getState().register({
    moduleId:    'wiki',
    routePrefix: '/wiki',
    placeholder: 'Search the wiki…',
    placeholderKey: 'wiki:search_ph',
    onSearch: (q) => {
      useWikiStore.getState().setSearchQuery(q)
      const id = getActiveWiki()
      if (id) goTo(`/wiki/${id}/search`)
    },
  })

  // Routes
  const WikiStartPage    = lazy(() => import('./WikiStartPage'))
  const WikiHome         = lazy(() => import('./WikiHome'))
  const PageView         = lazy(() => import('./PageView'))
  const PageEditor       = lazy(() => import('./PageEditor'))
  const SpecialView      = lazy(() => import('./SpecialView'))
  const CategoryView     = lazy(() => import('./CategoryView'))
  const SearchView       = lazy(() => import('./SearchView'))
  const WikiSettingsPage = lazy(() => import('./WikiSettingsPage'))

  RouteRegistry.register('wiki',                          WikiStartPage)
  RouteRegistry.register('wiki/settings',                 WikiSettingsPage)
  RouteRegistry.register('wiki/:wikiId',                  WikiHome)
  RouteRegistry.register('wiki/:wikiId/page/:ns/:title',  PageView)
  RouteRegistry.register('wiki/:wikiId/edit/:ns/:title',  PageEditor)
  RouteRegistry.register('wiki/:wikiId/special/:name',    SpecialView)
  RouteRegistry.register('wiki/:wikiId/category/:slug',   CategoryView)
  RouteRegistry.register('wiki/:wikiId/search',           SearchView)
  RouteRegistry.register('wiki/:wikiId/settings',         WikiSettingsPage)
}
