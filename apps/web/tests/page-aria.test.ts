import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const formPageFiles = [
  "app/auth/sign-in/page.tsx",
  "app/responsible-gaming/self-exclusion/page.tsx",
  "app/support/contact/page.tsx",
  "app/support/disputes/page.tsx",
  "app/topup/page.tsx",
];
const kycFormPageFiles = [
  "app/kyc/tier-1/page.tsx",
  "app/kyc/tier-2/page.tsx",
  "app/kyc/tier-3/page.tsx",
];
const actionFormPageFiles = [
  ...kycFormPageFiles,
  "app/responsible-gaming/self-exclusion/page.tsx",
];

function readSignInPageSource(): string {
  return readFileSync(join(process.cwd(), "app/auth/sign-in/page.tsx"), "utf8");
}

function readLandingPageSource(): string {
  return readFileSync(join(process.cwd(), "app/page.tsx"), "utf8");
}

function listSourceFiles(root: string): string[] {
  const files: string[] = [];

  for (const entry of readdirSync(join(process.cwd(), root), {
    withFileTypes: true,
  })) {
    const path = join(root, entry.name);

    if (entry.isDirectory()) {
      files.push(...listSourceFiles(path));
    } else if (/\.[cm]?[tj]sx?$/.test(entry.name)) {
      files.push(path);
    }
  }

  return files;
}

function signInFormSource(): string {
  const source = readSignInPageSource();
  const match = source.match(/function SigninForm[\s\S]*?function SignupForm/);

  assert.ok(match, "SigninForm source should be discoverable");

  return match[0];
}

test("static forms do not use unsupported aria-disabled attributes", () => {
  for (const file of formPageFiles) {
    const source = readFileSync(join(process.cwd(), file), "utf8");

    assert.doesNotMatch(source, /<form\b[^>]*\baria-disabled=/, file);
  }
});


test("sign-in page does not render the legacy MFA notice paragraph", () => {
  const source = readFileSync(
    join(process.cwd(), "app/auth/sign-in/page.tsx"),
    "utf8",
  );
  const messages = JSON.parse(
    readFileSync(join(process.cwd(), "messages/zh-Hant.json"), "utf8"),
  ) as {
    auth: {
      signIn: {
        signin: Record<string, unknown>;
      };
    };
  };

  assert.doesNotMatch(source, /signin\.mfaNotice/);
  assert.equal(messages.auth.signIn.signin.mfaNotice, undefined);
});

test("sign-in page does not render the legacy OTP submit note", () => {
  const source = readFileSync(
    join(process.cwd(), "app/auth/sign-in/page.tsx"),
    "utf8",
  );
  const messages = JSON.parse(
    readFileSync(join(process.cwd(), "messages/zh-Hant.json"), "utf8"),
  ) as {
    auth: {
      signIn: {
        signin: Record<string, unknown>;
      };
    };
  };

  assert.doesNotMatch(source, /signin\.submitNote/);
  assert.equal(messages.auth.signIn.signin.submitNote, undefined);
});

test("sign-in page does not render the compliance reminder section", () => {
  const source = readFileSync(
    join(process.cwd(), "app/auth/sign-in/page.tsx"),
    "utf8",
  );
  const messages = JSON.parse(
    readFileSync(join(process.cwd(), "messages/zh-Hant.json"), "utf8"),
  ) as {
    auth: {
      signIn: Record<string, unknown>;
    };
  };

  assert.doesNotMatch(source, /<ComplianceReminder\b/);
  assert.doesNotMatch(source, /function ComplianceReminder\b/);
  assert.equal(messages.auth.signIn.compliance, undefined);
});

test("sign-in hero does not render the sign-in kicker", () => {
  const source = readSignInPageSource();
  const messages = JSON.parse(
    readFileSync(join(process.cwd(), "messages/zh-Hant.json"), "utf8"),
  ) as {
    auth: {
      signIn: {
        hero: {
          signin: Record<string, unknown>;
        };
      };
    };
  };

  assert.doesNotMatch(source, /hero\.\$\{mode\}\.kicker/);
  assert.equal(messages.auth.signIn.hero.signin.kicker, undefined);
});

test("landing page does not render ProphTile kickers", () => {
  const source = readLandingPageSource();
  const messages = JSON.parse(
    readFileSync(join(process.cwd(), "messages/zh-Hant.json"), "utf8"),
  ) as {
    landing: {
      hero: Record<string, unknown>;
      neutral: Record<string, unknown>;
      points: Record<string, unknown>;
      responsibleGaming: Record<string, unknown>;
      start: Record<string, unknown>;
    };
  };

  assert.doesNotMatch(source, /\bkicker=/);

  for (const section of [
    messages.landing.hero,
    messages.landing.neutral,
    messages.landing.points,
    messages.landing.responsibleGaming,
    messages.landing.start,
  ]) {
    assert.equal(section.kicker, undefined);
  }
});

test("web app pages and components do not render kicker labels", () => {
  const sourceFiles = ["app", "components", "lib"]
    .flatMap(listSourceFiles)
    .sort();
  const offenders = sourceFiles.filter((file) => {
    const source = readFileSync(join(process.cwd(), file), "utf8");

    return /\bkicker\b/.test(source);
  });

  assert.deepEqual(offenders, []);
});

test("sign-in identifier field accepts email or phone login names", () => {
  const source = signInFormSource();

  assert.match(source, /name="identifier"/);
  assert.match(source, /type="text"/);
  assert.match(source, /autoComplete="username"/);
  assert.doesNotMatch(source, /name="email"/);
  assert.doesNotMatch(source, /type="email"/);
});

test("sign-in copy explains phone numbers may include or omit the leading zero", () => {
  const messages = JSON.parse(
    readFileSync(join(process.cwd(), "messages/zh-Hant.json"), "utf8"),
  ) as {
    auth: {
      signIn: {
        hero: {
          signin: {
            tagline: string;
          };
        };
        signin: {
          identifier?: Record<string, unknown>;
        };
      };
    };
  };

  const identifier = messages.auth.signIn.signin.identifier;

  assert.equal(
    messages.auth.signIn.hero.signin.tagline,
    "用註冊 Email 或手機號碼與密碼登入。",
  );
  assert.ok(identifier, "missing auth.signIn.signin.identifier copy");
  assert.equal(identifier.label, "Email 或手機號碼");
  assert.equal(identifier.placeholder, "Email 或手機號碼");
  assert.match(String(identifier.hint), /0912345698/);
  assert.match(String(identifier.hint), /912345698/);
});

test("action forms are not shipped disabled", () => {
  for (const file of actionFormPageFiles) {
    const source = readFileSync(join(process.cwd(), file), "utf8");

    assert.doesNotMatch(source, /<fieldset\b[^>]*\bdisabled\b/, file);
    assert.doesNotMatch(source, /<button\b[^>]*\bdisabled\b/, file);
  }
});

test("trade panel submit is not validity-disabled before hydration validation runs", () => {
  const source = readFileSync(
    join(process.cwd(), "components/trade-panel.tsx"),
    "utf8",
  );

  assert.doesNotMatch(source, /disabled=\{!isValid \|\| isSubmitting\}/);
});

test("trade panel uses an explicit click handler instead of native form submission", () => {
  const source = readFileSync(
    join(process.cwd(), "components/trade-panel.tsx"),
    "utf8",
  );

  assert.doesNotMatch(source, /type="submit"/);
  assert.match(source, /onClick=\{handleSubmit\(onSubmit\)\}/);
});

test("trade panel has a concrete default side for hydrated submission", () => {
  const source = readFileSync(
    join(process.cwd(), "components/trade-panel.tsx"),
    "utf8",
  );

  assert.match(source, /value="YES"[\s\S]*defaultChecked/);
});
