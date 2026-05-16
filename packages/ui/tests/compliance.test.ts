import assert from "node:assert/strict";
import test from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import {
  AgeConfirmationGate,
  KycEntryButton,
  NeutralPlatformDisclosure,
  ResponsibleGamingBanner,
  ResponsibleGamingModal,
} from "../src/compliance/index";

test("compliance components render required SPEC-021 content", () => {
  const html = renderToStaticMarkup(
    React.createElement(React.Fragment, null, [
      React.createElement(KycEntryButton, {
        href: "/auth/sign-in?mode=signup",
        key: "kyc",
      }),
      React.createElement(NeutralPlatformDisclosure, { key: "neutral" }),
      React.createElement(ResponsibleGamingBanner, {
        href: "/responsible-gaming",
        key: "rg-banner",
      }),
      React.createElement(ResponsibleGamingModal, {
        href: "/responsible-gaming",
        key: "rg-modal",
        open: true,
      }),
      React.createElement(AgeConfirmationGate, {
        confirmHref: "/",
        key: "age",
      }),
    ]),
  );

  assert.match(html, /開始驗證/);
  assert.match(html, /中立撮合，不做對手方/);
  assert.match(html, /理性參與，量力而為/);
  assert.match(html, /若你需要立即協助/);
  assert.match(html, /18 歲以上限定/);
  assert.match(html, /href="\/auth\/sign-in\?mode=signup"/);
  assert.match(html, /href="\/responsible-gaming"/);
});
