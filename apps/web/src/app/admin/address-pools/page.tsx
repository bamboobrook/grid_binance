import Link from "next/link";

export default function AdminAddressPoolsPage() {
  return (
    <main>
      <h1>Address Pool Expansion</h1>
      <p>Treasury sweep queue and hot wallet rotation status are visible for pool expansion.</p>
      <ul>
        <li>
          <Link href="/admin/dashboard">Dashboard</Link>
        </li>
        <li>
          <Link href="/admin/templates">Templates</Link>
        </li>
        <li>
          <Link href="/admin/billing">Billing Admin</Link>
        </li>
      </ul>
    </main>
  );
}
