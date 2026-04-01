import Link from "next/link";

const adminLinks = [
  { href: "/admin/users", label: "Member Control" },
  { href: "/admin/address-pools", label: "Address Pools" },
  { href: "/admin/templates", label: "Templates" },
  { href: "/admin/billing", label: "Billing Admin" },
  { href: "/admin/audit", label: "Audit Logs" },
];

export default function AdminDashboardPage() {
  return (
    <main>
      <h1>Admin Dashboard</h1>
      <p>Price config review is ready for operator approval and rollout.</p>
      <ul>
        {adminLinks.map((link) => (
          <li key={link.href}>
            <Link href={link.href}>{link.label}</Link>
          </li>
        ))}
      </ul>
    </main>
  );
}
