import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_SYNC_SUBNET = "172.30.20.0/24";

async function readText(root, relativePath) {
  return await readFile(path.join(root, relativePath), "utf8");
}

function includesRegex(text, regex) {
  return regex.test(text);
}

export async function checkPostgresSyncConfig(root = process.cwd()) {
  const compose = await readText(root, "infra/docker-compose.postgres-sync.yaml");
  const pgHba = await readText(root, "infra/postgres/tokyo-primary/pg_hba.conf");
  const replicationSql = await readText(
    root,
    "infra/postgres/tokyo-primary/init/001_replication.sql",
  );
  const errors = [];

  if (!compose.includes("${POSTGRES_PASSWORD:?POSTGRES_PASSWORD is required}")) {
    errors.push(
      "infra/docker-compose.postgres-sync.yaml: POSTGRES_PASSWORD must be required from the environment.",
    );
  }

  if (
    !compose.includes(
      "${POSTGRES_REPLICATION_PASSWORD:?POSTGRES_REPLICATION_PASSWORD is required}",
    )
  ) {
    errors.push(
      "infra/docker-compose.postgres-sync.yaml: POSTGRES_REPLICATION_PASSWORD must be required from the environment.",
    );
  }

  if (
    includesRegex(compose, /^\s*-\s*"?[0-9]+:5432"?\s*$/m) ||
    includesRegex(compose, /^\s*ports:\s*\[\s*"?[0-9]+:5432"?\s*\]/m)
  ) {
    errors.push(
      "infra/docker-compose.postgres-sync.yaml: Postgres host ports must bind to 127.0.0.1.",
    );
  }

  if (
    !compose.includes("127.0.0.1:${POSTGRES_PRIMARY_HOST_PORT:-55432}:5432") ||
    !compose.includes("127.0.0.1:${POSTGRES_REPLICA_HOST_PORT:-55433}:5432")
  ) {
    errors.push(
      "infra/docker-compose.postgres-sync.yaml: expected primary and replica ports to use loopback binds.",
    );
  }

  if (!compose.includes(`subnet: ${DEFAULT_SYNC_SUBNET}`)) {
    errors.push(
      `infra/docker-compose.postgres-sync.yaml: postgres-sync network must declare ${DEFAULT_SYNC_SUBNET}.`,
    );
  }

  if (pgHba.includes("0.0.0.0/0")) {
    errors.push("infra/postgres/tokyo-primary/pg_hba.conf: must not allow 0.0.0.0/0.");
  }

  if (!pgHba.includes(DEFAULT_SYNC_SUBNET)) {
    errors.push(
      `infra/postgres/tokyo-primary/pg_hba.conf: must allow only the configured local sync subnet ${DEFAULT_SYNC_SUBNET}.`,
    );
  }

  if (replicationSql.includes("replicator-dev-password")) {
    errors.push(
      "infra/postgres/tokyo-primary/init/001_replication.sql: must not commit a replication password.",
    );
  }

  if (!replicationSql.includes("\\getenv replication_password POSTGRES_REPLICATION_PASSWORD")) {
    errors.push(
      "infra/postgres/tokyo-primary/init/001_replication.sql: must read POSTGRES_REPLICATION_PASSWORD from the environment.",
    );
  }

  return { errors };
}

async function main() {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
  const { errors } = await checkPostgresSyncConfig(root);

  if (errors.length > 0) {
    for (const error of errors) {
      console.error(error);
    }
    process.exitCode = 1;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await main();
}
