// The account control at the foot of the rail: the user's avatar, opening a
// small popover with who they're signed in as and a sign-out action.
import { useEffect, useRef, useState } from "react";
import { LogOut } from "lucide-react";

import { strings } from "../i18n";
import { Avatar } from "../ds";
import { useAuth } from "../auth";
import styles from "./UserMenu.module.css";

export function UserMenu() {
  const { identity, signOut } = useAuth();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function onPointerDown(e: PointerEvent) {
      if (ref.current !== null && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const name = identity?.name ?? "";
  const email = identity?.email ?? "";

  return (
    <div className={styles.wrap} ref={ref}>
      <button
        type="button"
        className={styles.trigger}
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={strings.userMenu}
      >
        <Avatar name={name} email={email} size="md" />
      </button>

      {open && (
        <div className={styles.menu} role="menu">
          <div className={styles.identity}>
            <span className={styles.who}>{strings.signedInAs}</span>
            <span className={styles.name}>{name}</span>
            <span className={styles.email}>{email}</span>
          </div>
          <button
            type="button"
            className={styles.item}
            role="menuitem"
            onClick={() => {
              setOpen(false);
              void signOut();
            }}
          >
            <LogOut size={16} />
            <span>{strings.signOut}</span>
          </button>
        </div>
      )}
    </div>
  );
}
