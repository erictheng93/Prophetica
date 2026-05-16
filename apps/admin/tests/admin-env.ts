const parsedPort = Number.parseInt(process.env.ADMIN_PORT ?? "3002", 10);
const adminPort = Number.isNaN(parsedPort) || parsedPort <= 0 ? 3002 : parsedPort;

export const ADMIN_PORT = adminPort;
export const ADMIN_ORIGIN = `http://localhost:${adminPort}`;
