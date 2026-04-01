import Link from "next/link";

export default function AdminAuditPage() {
  return (
    <main>
      <h1>Audit Logs</h1>
      <p>Treasury sweep views, template actions, and membership approvals are retained here.</p>
      <ul>
        <li>
          <Link href="/admin/dashboard">Dashboard</Link>
        </li>
        <li>
          <Link href="/admin/users">Member Control</Link>
        </li>
        <li>
          <Link href="/admin/address-pools">Address Pools</Link>
        </li>
      </ul>
    </main>
  );
}
