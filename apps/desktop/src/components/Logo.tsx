/**
 * ContextSleep mark: a workspace window that holds a crescent.
 * Drawn as vector so it stays sharp in the 20-32px chrome where the raster
 * product thumbnail turned into mush. The packaged application icon is unchanged.
 */
export function Logo({size = 28, className = ''}: {size?: number; className?: string}) {
  return <svg className={`logo-mark ${className}`} width={size} height={size} viewBox="0 0 32 32"
    fill="none" role="img" aria-label="ContextSleep">
    <defs>
      <linearGradient id="cs-edge" x1="4" y1="3" x2="28" y2="29" gradientUnits="userSpaceOnUse">
        <stop stopColor="var(--logo-edge-1)" />
        <stop offset="1" stopColor="var(--logo-edge-2)" />
      </linearGradient>
      <linearGradient id="cs-moon" x1="10" y1="12" x2="23" y2="26" gradientUnits="userSpaceOnUse">
        <stop stopColor="var(--logo-moon-1)" />
        <stop offset="1" stopColor="var(--logo-moon-2)" />
      </linearGradient>
      <mask id="cs-crescent">
        <rect width="32" height="32" fill="#000" />
        <circle cx="16.4" cy="19" r="6.6" fill="#fff" />
        <circle cx="21.2" cy="15.6" r="5.9" fill="#000" />
      </mask>
    </defs>
    <rect x="2.6" y="3.4" width="26.8" height="25.2" rx="8" fill="var(--logo-body)" />
    <rect x="2.6" y="3.4" width="26.8" height="25.2" rx="8" stroke="url(#cs-edge)" strokeWidth="1.7" />
    <path d="M3.4 11h25.2" stroke="url(#cs-edge)" strokeWidth="1.2" opacity=".55" />
    <circle cx="8" cy="7.2" r="1.35" fill="url(#cs-edge)" opacity=".9" />
    <circle cx="12.6" cy="7.2" r="1.35" fill="url(#cs-edge)" opacity=".45" />
    <rect x="6" y="12" width="20" height="14" fill="url(#cs-moon)" mask="url(#cs-crescent)" />
  </svg>;
}

export function Wordmark({tag}: {tag?: string}) {
  return <span className="wordmark">
    <Logo size={30} />
    <span className="wordmark-text">
      <strong>ContextSleep</strong>
      {tag && <small>{tag}</small>}
    </span>
  </span>;
}
