// The sign-in screen. It owns the credential form (the IdP renders none) and
// maps the provider's outcomes to plain, human error text — revealing the
// authentication-code field only when the account has 2FA enrolled.
import { useState } from "react";
import type { FormEvent } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import { strings } from "../i18n";
import { Button, Spinner } from "../ds";
import { Logo } from "../shell/Logo";
import { useAuth } from "./AuthProvider";
import { AuthError } from "./oidcClient";
import styles from "./LoginPage.module.css";

export function LoginPage() {
  const { signIn } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();
  const redirectTo = (location.state as { from?: string } | null)?.from ?? "/mail";

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [otp, setOtp] = useState("");
  const [showOtp, setShowOtp] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      await signIn(email, password, showOtp ? otp : undefined);
      navigate(redirectTo, { replace: true });
    } catch (err) {
      if (err instanceof AuthError) {
        switch (err.kind) {
          case "second_factor":
            setShowOtp(true);
            setError(strings.errorSecondFactor);
            break;
          case "bad_credentials":
            setError(strings.errorBadCredentials);
            break;
          case "rate_limited":
            setError(strings.errorRateLimited);
            break;
          case "network":
            setError(strings.errorNetwork);
            break;
          default:
            setError(strings.errorGeneric);
        }
      } else {
        setError(strings.errorGeneric);
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className={styles.page}>
      <form className={styles.card} onSubmit={onSubmit}>
        <div className={styles.brand}>
          <Logo size={40} withWordmark />
        </div>
        <h1 className={styles.title}>{strings.loginTitle}</h1>
        <p className={styles.subtitle}>{strings.loginSubtitle}</p>

        <label className={styles.field}>
          <span className={styles.label}>{strings.emailLabel}</span>
          <input
            className={styles.input}
            type="email"
            autoComplete="username"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            autoFocus
          />
        </label>

        <label className={styles.field}>
          <span className={styles.label}>{strings.passwordLabel}</span>
          <input
            className={styles.input}
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </label>

        {showOtp && (
          <label className={styles.field}>
            <span className={styles.label}>{strings.otpLabel}</span>
            <input
              className={styles.input}
              type="text"
              inputMode="numeric"
              autoComplete="one-time-code"
              pattern="[0-9]*"
              maxLength={8}
              value={otp}
              onChange={(e) => setOtp(e.target.value)}
              autoFocus
              required
            />
            <span className={styles.hint}>{strings.otpHint}</span>
          </label>
        )}

        {error !== null && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}

        <Button type="submit" block disabled={submitting}>
          {submitting ? <Spinner size={16} label={strings.signingIn} /> : strings.signInButton}
        </Button>
      </form>
    </div>
  );
}
