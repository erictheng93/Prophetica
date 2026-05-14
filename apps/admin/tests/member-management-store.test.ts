import assert from "node:assert/strict";
import { beforeEach, test } from "node:test";
import {
  createMember,
  deleteMember,
  listMemberSummaries,
  resetMemberManagementStoreForTests,
  updateMember,
} from "../lib/member-management-store.js";
import type { SessionPayload } from "../lib/session-crypto.js";

const adminViewer: SessionPayload = {
  sub: "U-ADMIN-001",
  role: "admin",
  iat: 1,
  exp: 2,
};

const managerViewer: SessionPayload = {
  sub: "U-MGR-001",
  role: "manager",
  iat: 1,
  exp: 2,
};

const affiliateViewer: SessionPayload = {
  sub: "U-AFF-001",
  role: "affiliate",
  affiliateId: "AFF-001",
  iat: 1,
  exp: 2,
};

const otherAffiliateViewer: SessionPayload = {
  sub: "U-AFF-002",
  role: "affiliate",
  affiliateId: "AFF-014",
  iat: 1,
  exp: 2,
};

beforeEach(() => {
  resetMemberManagementStoreForTests();
});

test("admin and manager can list all members with source attribution", () => {
  const adminMembers = listMemberSummaries(adminViewer);
  const managerMembers = listMemberSummaries(managerViewer);

  assert.deepEqual(
    adminMembers.map((summary) => summary.member.user_id),
    ["MEM-001", "MEM-1042", "user_123"],
  );
  assert.deepEqual(
    managerMembers.map((summary) => summary.member.user_id),
    ["MEM-001", "MEM-1042", "user_123"],
  );
  assert.equal(adminMembers[0]?.source.affiliate_id, "AFF-001");
  assert.equal(adminMembers[1]?.source.affiliate_id, "AFF-014");
});

test("affiliate can only list members attributed to their affiliate scope", () => {
  assert.deepEqual(
    listMemberSummaries(affiliateViewer).map((summary) => summary.member.user_id),
    ["MEM-001", "user_123"],
  );
  assert.deepEqual(
    listMemberSummaries(otherAffiliateViewer).map((summary) => summary.member.user_id),
    ["MEM-1042"],
  );
});

test("creating a member defaults the account to inactive until KYC is green", () => {
  const result = createMember(
    {
      user_id: "MEM-NEW",
      display_name: "新會員",
      email: "new.member@example.com",
      phone: "0912345678",
      line_id: "new-line",
      country: "TW",
      timezone: "Asia/Taipei",
      tier: "Standard",
      affiliate_id: "AFF-001",
    },
    adminViewer,
  );

  assert.equal(result.ok, true);
  assert.equal(result.summary.member.status, "inactive");
  assert.equal(result.summary.kyc_risk.kyc_status, "pending");
  assert.equal(result.summary.kyc_risk.risk_level, "medium");
  assert.deepEqual(result.summary.kyc_risk.restrictions, ["kyc_pending"]);
  assert.equal(result.summary.source.affiliate_id, "AFF-001");
});

test("affiliate create forces the member source to the actor affiliate id", () => {
  const result = createMember(
    {
      user_id: "MEM-AFF",
      display_name: "代理會員",
      email: "aff.member@example.com",
      phone: "0912000111",
      country: "TW",
      timezone: "Asia/Taipei",
      tier: "Standard",
      affiliate_id: "AFF-014",
    },
    affiliateViewer,
  );

  assert.equal(result.ok, true);
  assert.equal(result.summary.source.affiliate_id, "AFF-001");
  assert.deepEqual(
    listMemberSummaries(otherAffiliateViewer).map((summary) => summary.member.user_id),
    ["MEM-1042"],
  );
});

test("updating KYC to green activates an inactive member", () => {
  createMember(
    {
      user_id: "MEM-KYC",
      display_name: "待審會員",
      email: "kyc.member@example.com",
      phone: "0912000222",
      country: "TW",
      timezone: "Asia/Taipei",
      tier: "Standard",
    },
    adminViewer,
  );

  const result = updateMember(
    "MEM-KYC",
    {
      display_name: "已審會員",
      kyc_status: "verified",
      risk_level: "low",
      risk_score: 12,
    },
    managerViewer,
  );

  assert.equal(result.ok, true);
  assert.equal(result.summary.member.display_name, "已審會員");
  assert.equal(result.summary.member.status, "active");
  assert.equal(result.summary.kyc_risk.kyc_status, "verified");
  assert.deepEqual(result.summary.kyc_risk.restrictions, []);
});

test("affiliate cannot edit members outside their affiliate scope", () => {
  const result = updateMember(
    "MEM-1042",
    { display_name: "越權修改" },
    affiliateViewer,
  );

  assert.deepEqual(result, { ok: false, status: 403, error: "forbidden" });
});

test("admin and manager can delete members but affiliate cannot", () => {
  assert.deepEqual(deleteMember("MEM-1042", affiliateViewer), {
    ok: false,
    status: 403,
    error: "forbidden",
  });

  assert.deepEqual(deleteMember("MEM-1042", managerViewer), {
    ok: true,
    user_id: "MEM-1042",
  });
  assert.equal(
    listMemberSummaries(adminViewer).some((summary) => summary.member.user_id === "MEM-1042"),
    false,
  );
});
