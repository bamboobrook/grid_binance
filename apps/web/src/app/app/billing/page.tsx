import Link from "next/link";

export default function BillingPage() {
  return (
    <main>
      <h1>Billing Center</h1>
      <p>Next renewal: 2026-04-15. Expiry reminder is queued if payment is still pending.</p>
      <ul>
        <li>
          <Link href="/app/security">Security Center</Link>
        </li>
        <li>
          <Link href="/app/strategies/grid-btc">Strategy Workspace</Link>
        </li>
        <li>
          <Link href="/help/expiry-reminder">Help Center</Link>
        </li>
      </ul>
    </main>
  );
}
