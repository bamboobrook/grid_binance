import Link from "next/link";

export default function AdminUsersPage() {
  return (
    <main>
      <h1>Member Control</h1>
      <p>Review membership overrides, grace status, and manual operator notes.</p>
      <ul>
        <li>
          <Link href="/admin/dashboard">Dashboard</Link>
        </li>
        <li>
          <Link href="/admin/address-pools">Address Pools</Link>
        </li>
        <li>
          <Link href="/admin/audit">Audit Logs</Link>
        </li>
      </ul>
    </main>
  );
}
