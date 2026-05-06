import { ProphLinkButton, ProphTile } from "@prophetica/ui";

export default function MarketNotFound() {
  return (
    <ProphTile
      surface="light"
      kicker="找不到這個市場"
      headline="這個市場可能已下架，或網址有誤。"
      tagline="請從市場列表重新進入，或聯繫客服。"
      actions={
        <>
          <ProphLinkButton variant="primary" href="/markets">
            回市場列表
          </ProphLinkButton>
          <ProphLinkButton variant="secondary-pill" href="/support">
            聯繫客服
          </ProphLinkButton>
        </>
      }
    />
  );
}
