// A heading block (ADR 0015). Headings are the document's sections, so they feed
// the numbering engine; the level switch (H1/H2) sets the nesting.
import { strings } from "../i18n";
import { cx } from "../ds";
import styles from "./HeadingBlock.module.css";

interface HeadingBlockProps {
  level: 1 | 2;
  text: string;
  /** The section number from the engine (e.g. "2.1"), shown as a quiet prefix. */
  number: string | undefined;
  onChange: (text: string) => void;
  onLevelChange: (level: 1 | 2) => void;
}

export function HeadingBlock({ level, text, number, onChange, onLevelChange }: HeadingBlockProps) {
  return (
    <div className={styles.row}>
      <div className={styles.level}>
        <button
          type="button"
          className={cx(styles.levelBtn, level === 1 && styles.levelOn)}
          onClick={() => onLevelChange(1)}
          title={strings.headingH1}
        >
          H1
        </button>
        <button
          type="button"
          className={cx(styles.levelBtn, level === 2 && styles.levelOn)}
          onClick={() => onLevelChange(2)}
          title={strings.headingH2}
        >
          H2
        </button>
      </div>
      <div className={cx(styles.headingWrap, level === 1 ? styles.h1 : styles.h2)}>
        {number !== undefined && <span className={styles.number}>{number}</span>}
        <input
          className={styles.input}
          value={text}
          placeholder={strings.headingPlaceholder}
          onChange={(e) => onChange(e.target.value)}
          aria-label={strings.headingLabel}
        />
      </div>
    </div>
  );
}
