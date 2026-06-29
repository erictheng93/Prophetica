import assert from "node:assert/strict";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { checkPostgresSyncConfig } from "./check-postgres-sync-config.mjs";

async function makeRoot() {
  return await mkdtemp(path.join(os.tmpdir(), "prophetica-postgres-sync-config-"));
}

async function writeConfig(root, { compose, pgHba, replicationSql }) {
  await mkdir(path.join(root, "infra/postgres/tokyo-primary/init"), { recursive: true });
  await writeFile(path.join(root, "infra/docker-compose.postgres-sync.yaml"), compose);
  await writeFile(path.join(root, "infra/postgres/tokyo-primary/pg_hba.conf"), pgHba);
  await writeFile(
    path.join(root, "infra/postgres/tokyo-primary/init/001_replication.sql"),
    replicationSql,
  );
}

const validCompose = `
services:
  tokyo-primary:
    environment:
      POSTGRES_PASSWORD: \${POSTGRES_PASSWORD:?POSTGRES_PASSWORD is required}
      POSTGRES_REPLICATION_PASSWORD: \${POSTGRES_REPLICATION_PASSWORD:?POSTGRES_REPLICATION_PASSWORD is required}
    ports:
      - "127.0.0.1:\${POSTGRES_PRIMARY_HOST_PORT:-55432}:5432"
    networks:
      - postgres-sync
  tokyo-replica:
    environment:
      POSTGRES_PASSWORD: \${POSTGRES_PASSWORD:?POSTGRES_PASSWORD is required}
      POSTGRES_REPLICATION_PASSWORD: \${POSTGRES_REPLICATION_PASSWORD:?POSTGRES_REPLICATION_PASSWORD is required}
    ports:
      - "127.0.0.1:\${POSTGRES_REPLICA_HOST_PORT:-55433}:5432"
    networks:
      - postgres-sync
networks:
  postgres-sync:
    ipam:
      config:
        - subnet: 172.30.20.0/24
`;

const validPgHba = `
local   all             all                                     trust
host    all             all             127.0.0.1/32            scram-sha-256
host    all             all             172.30.20.0/24          scram-sha-256
host    replication     replicator      127.0.0.1/32            scram-sha-256
host    replication     replicator      172.30.20.0/24          scram-sha-256
`;

const validReplicationSql = `
\\getenv replication_password POSTGRES_REPLICATION_PASSWORD
\\if :{?replication_password}
\\else
  \\echo 'POSTGRES_REPLICATION_PASSWORD is required'
  \\quit 1
\\endif
`;

test("accepts hardened postgres sync config", async () => {
  const root = await makeRoot();
  await writeConfig(root, {
    compose: validCompose,
    pgHba: validPgHba,
    replicationSql: validReplicationSql,
  });

  const { errors } = await checkPostgresSyncConfig(root);

  assert.deepEqual(errors, []);
});

test("rejects committed passwords, public binds, and open pg_hba ranges", async () => {
  const root = await makeRoot();
  await writeConfig(root, {
    compose: `
services:
  tokyo-primary:
    environment:
      POSTGRES_PASSWORD: dev
    ports: ["55432:5432"]
`,
    pgHba: `
host    all             all             0.0.0.0/0               scram-sha-256
host    replication     replicator      0.0.0.0/0               scram-sha-256
`,
    replicationSql:
      "CREATE ROLE replicator WITH REPLICATION LOGIN PASSWORD 'replicator-dev-password';\n",
  });

  const { errors } = await checkPostgresSyncConfig(root);

  assert.deepEqual(errors, [
    "infra/docker-compose.postgres-sync.yaml: POSTGRES_PASSWORD must be required from the environment.",
    "infra/docker-compose.postgres-sync.yaml: POSTGRES_REPLICATION_PASSWORD must be required from the environment.",
    "infra/docker-compose.postgres-sync.yaml: Postgres host ports must bind to 127.0.0.1.",
    "infra/docker-compose.postgres-sync.yaml: expected primary and replica ports to use loopback binds.",
    "infra/docker-compose.postgres-sync.yaml: postgres-sync network must declare 172.30.20.0/24.",
    "infra/postgres/tokyo-primary/pg_hba.conf: must not allow 0.0.0.0/0.",
    "infra/postgres/tokyo-primary/pg_hba.conf: must allow only the configured local sync subnet 172.30.20.0/24.",
    "infra/postgres/tokyo-primary/init/001_replication.sql: must not commit a replication password.",
    "infra/postgres/tokyo-primary/init/001_replication.sql: must read POSTGRES_REPLICATION_PASSWORD from the environment.",
  ]);
});
