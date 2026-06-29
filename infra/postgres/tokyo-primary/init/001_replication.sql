\getenv replication_password POSTGRES_REPLICATION_PASSWORD
\if :{?replication_password}
\else
  \echo 'POSTGRES_REPLICATION_PASSWORD is required'
  \quit 1
\endif

SELECT format(
  'CREATE ROLE replicator WITH REPLICATION LOGIN PASSWORD %L',
  :'replication_password'
)
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'replicator')
\gexec

ALTER ROLE replicator WITH REPLICATION LOGIN PASSWORD :'replication_password';

SELECT pg_create_physical_replication_slot('standby1')
WHERE NOT EXISTS (
  SELECT 1 FROM pg_replication_slots WHERE slot_name = 'standby1'
);

SELECT pg_create_physical_replication_slot('singapore_dr')
WHERE NOT EXISTS (
  SELECT 1 FROM pg_replication_slots WHERE slot_name = 'singapore_dr'
);
