//! Admin bootstrap and client registration — the provisioning path used by
//! the `identityctl` CLI. Creating the first admin of a tenant is a
//! deliberately **non-public** operation (there is no HTTP signup surface):
//! a deployment operator runs it, and the password arrives from stdin or
//! the environment, never a command-line argument (which would leak to the
//! process table).

use ficina_store::{TenantId, UserId};

use crate::{Identity, Result};

/// The identities created by an admin bootstrap.
pub struct AdminAccount {
    /// The new tenant.
    pub tenant: TenantId,
    /// The tenant's first admin user.
    pub user: UserId,
}

impl Identity {
    /// Creates a tenant and its first admin user with a login and an inbox.
    /// The login username is the admin's email. Non-public — CLI/operator
    /// only.
    ///
    /// # Errors
    /// [`crate::IdentityError::Store`] if the tenant/user cannot be created;
    /// [`crate::IdentityError::Crypto`] on a hashing failure.
    pub async fn bootstrap_admin(
        &self,
        tenant_name: &str,
        email: &str,
        password: &str,
    ) -> Result<AdminAccount> {
        let tenant = self.store().create_tenant(tenant_name).await?;
        let user = self
            .store()
            .for_tenant(tenant.clone())
            .create_user(email)
            .await?;
        self.set_password(&tenant, &user, email, password).await?;
        // The bootstrap user is the tenant admin (gates the admin console).
        self.store()
            .for_tenant(tenant.clone())
            .set_admin(&user, true)
            .await?;
        // Give the admin a usable mailbox for dogfooding.
        self.store()
            .for_account(tenant.clone(), user.clone())
            .inbox()
            .await?;
        Ok(AdminAccount { tenant, user })
    }

    /// Registers (or replaces) a deployment-wide first-party **public**
    /// client (PKCE, no secret) — e.g. the web app. Idempotent on
    /// `client_id`.
    ///
    /// # Errors
    /// [`crate::IdentityError::Store`] on a persistence failure.
    pub async fn register_public_client(
        &self,
        client_id: &str,
        name: &str,
        redirect_uris: &[String],
    ) -> Result<()> {
        self.store()
            .register_oauth_client(client_id, None, name, redirect_uris, None)
            .await?;
        Ok(())
    }
}
