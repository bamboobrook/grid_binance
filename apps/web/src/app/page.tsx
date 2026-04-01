import Link from "next/link";

export default function HomePage() {
  return (
    <main>
      <h1>Grid Binance</h1>
      <p>Minimal user entry for registration, billing review, strategy workspace, and help flows.</p>
      <ul>
        <li>
          <Link href="/register">Registration Entry</Link>
        </li>
        <li>
          <Link href="/app/dashboard">Open User Dashboard</Link>
        </li>
        <li>
          <Link href="/help/expiry-reminder">Expiry Reminder Guide</Link>
        </li>
      </ul>
    </main>
  );
}
