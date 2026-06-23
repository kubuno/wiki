import React, { useState } from 'react'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { BookMarked, ArrowLeft, ExternalLink, Check } from 'lucide-react'
import { Toggle, Button, Radio } from '@ui'
import { useModulePrefs } from './userPrefs'

// ── Per-user preferences (backend, cross-device via core users.preferences) ─────

interface WikiPrefs {
  readingWidth: string   // 'narrow' | 'wide'
  showToc:      boolean
  fontSize:     string   // 'small' | 'normal' | 'large'
  editorTheme:  string   // 'light' | 'dark'
  livePreview:  boolean
  showRedLinks: boolean
}

const DEFAULT_PREFS: WikiPrefs = {
  readingWidth: 'narrow', showToc: true, fontSize: 'normal',
  editorTheme: 'light', livePreview: true, showRedLinks: true,
}

// ── Mail-style layout helpers ───────────────────────────────────────────────────

function SettingsRow({ label, description, children }: {
  label: string; description?: string; children: React.ReactNode
}) {
  return (
    <div className="flex items-start gap-8 py-4 border-b border-[#e8eaed] last:border-0">
      <div className="w-60 flex-shrink-0">
        <p className="text-sm text-[#202124] font-normal">{label}</p>
        {description && <p className="text-xs text-text-tertiary mt-0.5 leading-relaxed">{description}</p>}
      </div>
      <div className="flex-1">{children}</div>
    </div>
  )
}

function RadioGroup({ options, value, onChange }: {
  options: { value: string; label: string }[]; value: string; onChange: (v: string) => void
}) {
  return (
    <div className="flex flex-col items-start gap-2">
      {options.map(opt => (
        <Radio key={opt.value} checked={value === opt.value} onChange={() => onChange(opt.value)} label={opt.label} />
      ))}
    </div>
  )
}

// ── Préférences tab (per-user) ──────────────────────────────────────────────────

function PreferencesTab() {
  const { t } = useTranslation('wiki')
  const { prefs: saved, update } = useModulePrefs<WikiPrefs>('wiki', DEFAULT_PREFS)
  const [prefs, setPrefs] = useState<WikiPrefs>(saved)
  const [savedFlag, setSavedFlag] = useState(false)
  const [busy, setBusy] = useState(false)

  const set = <K extends keyof WikiPrefs>(key: K, value: WikiPrefs[K]) =>
    setPrefs(p => ({ ...p, [key]: value }))

  const save = async () => {
    setBusy(true)
    try {
      await update(prefs)
      setSavedFlag(true)
      setTimeout(() => setSavedFlag(false), 2500)
    } finally { setBusy(false) }
  }

  return (
    <div>
      <SettingsRow
        label={t('wiki_pref_reading_width', { defaultValue: 'Largeur de lecture' })}
        description={t('wiki_pref_reading_width_desc', { defaultValue: 'Largeur de la colonne de texte des articles.' })}
      >
        <RadioGroup
          value={prefs.readingWidth}
          onChange={v => set('readingWidth', v)}
          options={[
            { value: 'narrow', label: t('wiki_pref_reading_width_narrow', { defaultValue: 'Étroite (plus lisible)' }) },
            { value: 'wide',   label: t('wiki_pref_reading_width_wide',   { defaultValue: 'Large (pleine page)' }) },
          ]}
        />
      </SettingsRow>

      <SettingsRow
        label={t('wiki_pref_font_size', { defaultValue: 'Taille de police' })}
        description={t('wiki_pref_font_size_desc', { defaultValue: 'Taille du texte dans les articles.' })}
      >
        <RadioGroup
          value={prefs.fontSize}
          onChange={v => set('fontSize', v)}
          options={[
            { value: 'small',  label: t('wiki_pref_font_size_small',  { defaultValue: 'Petite' }) },
            { value: 'normal', label: t('wiki_pref_font_size_normal', { defaultValue: 'Normale' }) },
            { value: 'large',  label: t('wiki_pref_font_size_large',  { defaultValue: 'Grande' }) },
          ]}
        />
      </SettingsRow>

      <SettingsRow
        label={t('wiki_pref_editor_theme', { defaultValue: 'Thème de l\'éditeur' })}
        description={t('wiki_pref_editor_theme_desc', { defaultValue: 'Apparence de la zone d\'édition wikitexte.' })}
      >
        <RadioGroup
          value={prefs.editorTheme}
          onChange={v => set('editorTheme', v)}
          options={[
            { value: 'light', label: t('wiki_pref_editor_theme_light', { defaultValue: 'Clair' }) },
            { value: 'dark',  label: t('wiki_pref_editor_theme_dark',  { defaultValue: 'Sombre' }) },
          ]}
        />
      </SettingsRow>

      <SettingsRow label={t('wiki_pref_toc', { defaultValue: 'Table des matières' })}>
        <label className="flex items-center gap-2 cursor-pointer select-none">
          <Toggle checked={prefs.showToc} onChange={() => set('showToc', !prefs.showToc)} />
          <span className="text-sm text-text-primary">{t('wiki_pref_toc_on', { defaultValue: 'Afficher la table des matières des articles' })}</span>
        </label>
      </SettingsRow>

      <SettingsRow label={t('wiki_pref_live_preview', { defaultValue: 'Prévisualisation' })}>
        <label className="flex items-center gap-2 cursor-pointer select-none">
          <Toggle checked={prefs.livePreview} onChange={() => set('livePreview', !prefs.livePreview)} />
          <span className="text-sm text-text-primary">{t('wiki_pref_live_preview_on', { defaultValue: 'Prévisualisation en direct pendant l\'édition' })}</span>
        </label>
      </SettingsRow>

      <SettingsRow
        label={t('wiki_pref_red_links', { defaultValue: 'Liens rouges' })}
        description={t('wiki_pref_red_links_desc', { defaultValue: 'Affiche en rouge les liens vers des pages inexistantes.' })}
      >
        <label className="flex items-center gap-2 cursor-pointer select-none">
          <Toggle checked={prefs.showRedLinks} onChange={() => set('showRedLinks', !prefs.showRedLinks)} />
          <span className="text-sm text-text-primary">{t('wiki_pref_red_links_on', { defaultValue: 'Afficher les liens rouges' })}</span>
        </label>
      </SettingsRow>

      <div className="pt-5 flex items-center gap-3">
        <Button onClick={save} loading={busy}>
          {savedFlag
            ? <><Check size={14} className="mr-1.5 inline" />{t('wiki_settings_saved', { defaultValue: 'Enregistré' })}</>
            : t('wiki_settings_save_changes', { defaultValue: 'Enregistrer les modifications' })}
        </Button>
        <Button variant="ghost" onClick={() => setPrefs(saved)}>
          {t('common_cancel', { defaultValue: 'Annuler' })}
        </Button>
      </div>
    </div>
  )
}

// ── À propos tab ────────────────────────────────────────────────────────────────

function AboutTab() {
  const { t } = useTranslation('wiki')
  return (
    <div className="rounded-xl border border-border overflow-hidden">
      <div className="flex items-center gap-3 px-5 py-4 border-b border-border bg-surface-1">
        <div className="w-10 h-10 rounded-xl bg-emerald-100 flex items-center justify-center shrink-0">
          <BookMarked size={20} className="text-emerald-600" />
        </div>
        <div>
          <p className="text-sm font-semibold text-text-primary">Kubuno Wiki</p>
          <p className="text-xs text-text-tertiary">v0.1.0 · {t('wiki_official_module', { defaultValue: 'Module officiel' })}</p>
        </div>
        <span className="ml-auto text-xs font-medium px-2 py-0.5 rounded-full bg-orange-100 text-orange-700">Rust</span>
      </div>
      <div className="px-5 py-4">
        <a href="https://github.com/kubuno/wiki" target="_blank" rel="noopener noreferrer"
          className="inline-flex items-center gap-1.5 text-sm text-primary hover:underline">
          <ExternalLink size={13} /> github.com/kubuno/wiki
        </a>
      </div>
    </div>
  )
}

// ── Main page (mail-style breadcrumb + tab bar) ─────────────────────────────────

type Tab = 'preferences' | 'about'

export default function WikiUserSettingsPage() {
  const { t } = useTranslation('wiki')
  const [tab, setTab] = useState<Tab>('preferences')

  const tabs: { id: Tab; label: string }[] = [
    { id: 'preferences', label: t('wiki_tab_preferences', { defaultValue: 'Préférences' }) },
    { id: 'about',       label: t('wiki_tab_about', { defaultValue: 'À propos' }) },
  ]

  return (
    <div className="flex flex-col h-full bg-white overflow-hidden">
      {/* Breadcrumb header */}
      <div className="flex items-center gap-2 px-6 py-2.5 border-b border-[#e8eaed] flex-shrink-0" style={{ background: '#f8f9fa' }}>
        <Link to="/wiki" className="flex items-center gap-1.5 text-sm text-[#1a73e8] hover:underline">
          <ArrowLeft size={14} />
          Wiki
        </Link>
        <span className="text-text-tertiary text-sm">/</span>
        <div className="flex items-center gap-1.5">
          <BookMarked size={15} className="text-text-secondary" />
          <span className="text-sm text-text-primary">{t('wiki_settings_title', { defaultValue: 'Réglages' })}</span>
        </div>
      </div>

      {/* Tab bar (Gmail-style) */}
      <div className="flex items-end border-b border-[#e8eaed] px-4 flex-shrink-0 overflow-x-auto" style={{ background: '#fff' }}>
        {tabs.map(tb => (
          <button key={tb.id} onClick={() => setTab(tb.id)}
            className={`px-4 py-3 text-sm border-b-2 -mb-px transition-colors whitespace-nowrap ${
              tab === tb.id ? 'border-[#1a73e8] text-[#1a73e8] font-medium' : 'border-transparent text-[#5f6368] hover:text-[#202124] hover:bg-[#f1f3f4]'}`}>
            {tb.label}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-3xl mx-auto px-8 py-6">
          {tab === 'preferences' && <PreferencesTab />}
          {tab === 'about'       && <AboutTab />}
        </div>
      </div>
    </div>
  )
}
