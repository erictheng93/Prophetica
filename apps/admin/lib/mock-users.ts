import "server-only";

/*
 * Phase 2 mock users so admin/manager/affiliate flows can be exercised
 * end-to-end before user-service grows real auth endpoints.
 *
 * Replaced by a proxy to user-service in Phase 3 (see TODO.md). Until then,
 * passwords are intentionally obvious so they cannot be mistaken for
 * production credentials.
 */

export { findMockUser, mockUsers, type MockUser } from "./mock-users-data";
