import Link from "next/link";
import { notFound } from "next/navigation";

const validStrategyIds = new Set(["grid-btc"]);

function formatStrategyId(id: string) {
  return id
    .split("-")
    .filter(Boolean)
    .map((part) => part.toUpperCase())
    .join(" / ");
}

export default async function StrategyDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  if (!validStrategyIds.has(id)) {
    notFound();
  }

  return (
    <main>
      <h1>Strategy Workspace</h1>
      <p>Review strategy: {formatStrategyId(id)}</p>
      <ul>
        <li>
          <Link href="/app/analytics">Analytics</Link>
        </li>
        <li>
          <Link href="/help/expiry-reminder">Help Center</Link>
        </li>
        <li>
          <Link href="/app/dashboard">Back to Dashboard</Link>
        </li>
      </ul>
    </main>
  );
}
