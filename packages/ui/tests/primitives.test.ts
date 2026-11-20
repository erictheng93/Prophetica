import assert from "node:assert/strict";
import test from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import {
  AmbientGlow,
  NumberTicker,
  ProphBadge,
  ProphButton,
  ProphTile,
  ShimmerSkeleton,
  TiltCard,
} from "../src/index";

test("AmbientGlow renders with specified variant and size", () => {
  const html = renderToStaticMarkup(
    React.createElement(AmbientGlow, { variant: "gold", size: "lg" }),
  );
  assert.match(html, /from-\[#F0D78C\]/);
  assert.match(html, /h-\[480px\]/);
});

test("TiltCard renders children and spotlight container", () => {
  const html = renderToStaticMarkup(
    React.createElement(
      TiltCard,
      { className: "custom-card" },
      React.createElement("span", null, "Card Content"),
    ),
  );
  assert.match(html, /custom-card/);
  assert.match(html, /Card Content/);
  assert.match(html, /radial-gradient/);
});

test("NumberTicker renders formatted numbers with sign and suffix", () => {
  const html = renderToStaticMarkup(
    React.createElement(NumberTicker, {
      value: 1250,
      prefix: "$",
      suffix: " TWD",
      showSign: true,
    }),
  );
  assert.match(html, /\$\+1,250 TWD/);
});

test("ProphBadge renders with pulse pip and tone styling", () => {
  const html = renderToStaticMarkup(
    React.createElement(
      ProphBadge,
      { tone: "brand", pulse: true },
      "LIVE MARKET",
    ),
  );
  assert.match(html, /LIVE MARKET/);
  assert.match(html, /animate-ping/);
});

test("ShimmerSkeleton renders with animation classes", () => {
  const html = renderToStaticMarkup(
    React.createElement(ShimmerSkeleton, { className: "h-6 w-32" }),
  );
  assert.match(html, /proph-shimmer/);
  assert.match(html, /h-6 w-32/);
});

test("ProphButton renders standard button element with gold variant", () => {
  const html = renderToStaticMarkup(
    React.createElement(
      ProphButton,
      { variant: "gold-primary", type: "submit" },
      "儲值立即生效",
    ),
  );
  assert.match(html, /<button/);
  assert.match(html, /type="submit"/);
  assert.match(html, /儲值立即生效/);
  assert.match(html, /bg-gradient-to-b/);
});

test("ProphTile renders with ambient glow and grid pattern", () => {
  const html = renderToStaticMarkup(
    React.createElement(ProphTile, {
      headline: "頂級預測市場",
      glow: "brand",
      gridPattern: true,
    }),
  );
  assert.match(html, /頂級預測市場/);
  assert.match(html, /proph-grid-pattern/);
  assert.match(html, /rgba\(0, 102, 204, 0.15\)/);
});
