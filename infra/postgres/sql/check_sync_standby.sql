SELECT
  application_name,
  state,
  sync_state,
  write_lag,
  flush_lag,
  replay_lag
FROM pg_stat_replication
WHERE application_name = 'standby1';
