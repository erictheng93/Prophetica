DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'replicator') THEN
    CREATE ROLE replicator WITH REPLICATION LOGIN PASSWORD 'replicator-dev-password';
  END IF;
END
$$;

SELECT pg_create_physical_replication_slot('standby1')
WHERE NOT EXISTS (
  SELECT 1 FROM pg_replication_slots WHERE slot_name = 'standby1'
);

SELECT pg_create_physical_replication_slot('singapore_dr')
WHERE NOT EXISTS (
  SELECT 1 FROM pg_replication_slots WHERE slot_name = 'singapore_dr'
);
