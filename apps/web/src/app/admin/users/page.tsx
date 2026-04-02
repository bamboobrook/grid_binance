import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { DataTable } from "../../../components/ui/table";
import { getCurrentAdminProductState } from "../../../lib/api/admin-product-state";

type UsersPageProps = {
  searchParams?: Promise<{
    query?: string;
    state?: string;
  }>;
};

export default async function AdminUsersPage({ searchParams }: UsersPageProps) {
  const params = (await searchParams) ?? {};
  const query = typeof params.query === "string" ? params.query.trim().toLowerCase() : "";
  const stateFilter = typeof params.state === "string" ? params.state : "all";
  const state = await getCurrentAdminProductState();
  const rows = state.memberships.filter((item) => {
    const matchesState = stateFilter === "all" ? true : item.status.toLowerCase() === stateFilter;
    const matchesQuery = query.length === 0 ? true : item.email.toLowerCase().includes(query);
    return matchesState && matchesQuery;
  });

  return (
    <>
      <AppShellSection
        description="Search user accounts, review membership state, and jump into override surfaces without losing account context."
        eyebrow="User operations"
        title="User Management"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Find users</CardTitle>
              <CardDescription>Search by email and membership status.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/admin/users" method="get">
                <Field label="Email search">
                  <Input defaultValue={query} name="query" placeholder="miles@example.com" />
                </Field>
                <Field label="Membership state">
                  <Select defaultValue={stateFilter} name="state">
                    <option value="all">All states</option>
                    <option value="active">Active</option>
                    <option value="grace">Grace</option>
                    <option value="frozen">Frozen</option>
                    <option value="revoked">Revoked</option>
                  </Select>
                </Field>
                <ButtonRow>
                  <Button type="submit">Apply filters</Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Operator guidance</CardTitle>
              <CardDescription>Membership overrides and deposit decisions always remain auditable.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>Use Memberships for direct extend/freeze/unfreeze workflows.</li>
                <li>Use Deposits when billing mismatches block entitlement changes.</li>
                <li>Current result count: {rows.length}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>User state overview</CardTitle>
          <CardDescription>Membership, grace timing, and operator notes stay in one table.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "email", label: "Email" },
              { key: "plan", label: "Plan" },
              { key: "status", label: "Status" },
              { key: "note", label: "Operator note" },
              { key: "actions", label: "Actions", align: "right" },
            ]}
            rows={rows.map((item) => ({
              id: item.id,
              actions: <Link href="/admin/memberships">Open membership actions</Link>,
              email: item.email,
              note: item.note,
              plan: item.plan,
              status: <Chip tone={item.status === "Active" ? "success" : item.status === "Grace" ? "warning" : "danger"}>{item.status}</Chip>,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
