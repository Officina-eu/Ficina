// The Ficina mark: a verdigris workshop tile with a paper "F" and a single
// copper rivet — the warm-workshop identity in one glyph. `withWordmark`
// shows the name beside it (used on the login screen).
import styles from "./Logo.module.css";

interface LogoProps {
  size?: number;
  withWordmark?: boolean;
}

export function Logo({ size = 32, withWordmark = false }: LogoProps) {
  return (
    <span className={styles.logo}>
      <svg
        width={size}
        height={size}
        viewBox="0 0 32 32"
        role="img"
        aria-label="Ficina"
        fill="none"
      >
        <rect width="32" height="32" rx="8" fill="var(--verdigris-500)" />
        {/* F stem + arms in paper */}
        <rect x="10.5" y="8" width="3" height="16" rx="1" fill="var(--paper)" />
        <rect x="10.5" y="8" width="12" height="3" rx="1" fill="var(--paper)" />
        <rect x="10.5" y="14.5" width="8.5" height="3" rx="1" fill="var(--paper)" />
        {/* copper rivet */}
        <circle cx="22" cy="22" r="2" fill="var(--copper-500)" />
      </svg>
      {withWordmark && <span className={styles.wordmark}>Ficina</span>}
    </span>
  );
}
