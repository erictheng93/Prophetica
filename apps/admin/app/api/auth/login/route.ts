import { NextResponse } from "next/server";
import { findMockUser } from "@/lib/mock-users";
import { defaultLandingFor } from "@/lib/rbac";
import { setSessionCookie } from "@/lib/session";

interface LoginBody {
  email?: unknown;
  password?: unknown;
}

export async function POST(request: Request) {
  let body: LoginBody;
  try {
    body = (await request.json()) as LoginBody;
  } catch {
    return NextResponse.json({ error: "invalid_json" }, { status: 400 });
  }
  const email = typeof body.email === "string" ? body.email : "";
  const password = typeof body.password === "string" ? body.password : "";

  if (!email || !password) {
    return NextResponse.json({ error: "missing_credentials" }, { status: 400 });
  }

  const user = findMockUser(email, password);
  if (!user) {
    return NextResponse.json({ error: "invalid_credentials" }, { status: 401 });
  }

  await setSessionCookie({
    sub: user.sub,
    role: user.role,
    ...(user.affiliateId ? { affiliateId: user.affiliateId } : {}),
  });

  return NextResponse.json({
    role: user.role,
    redirect: defaultLandingFor(user.role),
  });
}
