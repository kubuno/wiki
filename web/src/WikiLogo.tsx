interface WikiLogoProps {
  size?:      number
  className?: string
  title?:     string
}

/** Wiki logo (designer artwork, raster). Served by the host from
 *  `/wiki-logo.png`; rendered as a square image so it weighs the same as its
 *  neighbours in the waffle menu. */
export function WikiLogo({ size = 24, className, title = 'Wiki' }: WikiLogoProps) {
  return (
    <img
      src="/wiki-logo.png"
      width={size}
      height={size}
      alt={title}
      className={className}
      style={{ display: 'block', objectFit: 'contain' }}
    />
  )
}

export default WikiLogo
