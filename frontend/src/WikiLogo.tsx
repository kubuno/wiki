/** Wiki module glyph — used by the waffle menu and the favicon. */
export default function WikiLogo({ size = 24 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect x="3" y="3" width="18" height="18" rx="4" fill="#0f766e" />
      <path
        d="M6 8.5l1.7 7h.1l1.6-5.2h.1L11.2 15.5h.1l1.7-7"
        stroke="#fff"
        strokeWidth="1.4"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
      <path d="M15 8.5h3M16.5 8.5V15.5M15 15.5h3" stroke="#9decd9" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  )
}
