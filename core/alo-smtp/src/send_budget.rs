//! Outbound send budget: hourly caps on queue deliveries, global and
//! per envelope sender. This is IP-reputation protection — a
//! compromised account (or a runaway integration) mass-mailing through
//! us would burn the deployment's sending reputation in minutes;
//! capped, it turns into a throttled queue and a log signal instead.
//!
//! Over-budget messages are **deferred, never bounced**: they stay in
//! the sp