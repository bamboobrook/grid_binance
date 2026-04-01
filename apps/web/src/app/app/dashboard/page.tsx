import Link from "next/link";

const quickLinks = [
  { href: "/app/billing", label: "Billing Center" },
  { href: "/app/security", label: "Security Center" },
  { href: "/app/strategies/grid-btc", label: "Strategy Workspace" },
  { href: "/app/analytics", label: "Analytics" },
  { href: "/help/expiry-reminder", label: "Help Center" },
];

export default function DashboardPage() {
  return (
    <main>
      <h1>User Dashboard</h1>
      <p>Expiry reminder flow is active for your current membership cycle.</p>
      <ul>
        {quickLinks.map((link) => (
          <li key={link.href}>
            <Link href={link.href}>{link.label}</Link>
          </li>
        ))}
      </ul>
    </main>
  );
}
