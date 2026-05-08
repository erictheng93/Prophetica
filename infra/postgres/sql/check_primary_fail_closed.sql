SELECT
  pg_is_in_recovery() AS in_recovery,
  current_setting('synchronous_commit') AS synchronous_commit,
  current_setting('synchronous_standby_names') AS synchronous_standby_names;
