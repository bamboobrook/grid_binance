import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminMembershipsData } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ action?: string; email?: string }>;
};

export default async function AdminMembershipsPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const selectedEmail = typeof params.email === "string" ? params.email : "";
  const lastAction = typeof params.action === "string" ? params.action : "";
  const data = await getAdminMembershipsData();
  const selected = data.items.find((item) => item.email === selectedEmail) ?? null;

  return (
    <>
      {selected && lastAction ? (
        <StatusBanner
          description={`Target: ${selected.email} | Status: ${selected.status} | Last action: ${lastAction}`}
          title="Membership updated"
          tone="success"
        />
      ) : null}
      <AppShellSection
        description="Backend-backed membership operations for open, extend, freeze, unfreeze, and revoke."
        eyebrow="Membership operations"
        title="Membership Operations"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Manual membership controls</CardTitle>
              <CardDescription>Operate directly against backend membership actions.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/admin/memberships" method="post">
                <Field label="Member email">
                  <Input defaultValue={selectedEmail} name="email" placeholder="member@example.com" />
                </Field>
                <Field label="Duration days">
                  <Input defaultValue="30" inputMode="numeric" name="durationDays" />
                </Field>
                <input name="action" type="hidden" value="open" />
                <Button type="submit">Open membership</Button>
              </FormStack>
              <FormStack action="/api/admin/memberships" method="post">
                <input name="email" type="hidden" value={selectedEmail} />
                <input name="durationDays" type="hidden" value="15" />
                <input name="action" type="hidden" value="extend" />
                <Button type="submit">Extend membership</Button>
              </FormStack>
              {selected?.status === "Frozen" ? (
                <FormStack action="/api/admin/memberships" method="post">
                  <input name="email" type="hidden" value={selectedEmail} />
                  <input name="action" type="hidden" value="unfreeze" />
                  <Button type="submit">Unfreeze membership</Button>
                </FormStack>
              ) : (
                <FormStack action="/api/admin/memberships" method="post">
                  <input name="email" type="hidden" value={selectedEmail} />
                  <input name="action" type="hidden" value="freeze" />
                  <Button type="submit">Freeze membership</Button>
                </FormStack>
              )}
              <FormStack action="/api/admin/memberships" method="post">
                <input name="email" type="hidden" value={selectedEmail} />
                <input name="action" type="hidden" value="revoke" />
                <Button type="submit">Revoke membership</Button>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Selected member</CardTitle>
              <CardDescription>Latest backend result for the focused member.</CardDescription>
            </CardHeader>
            <CardBody>
              {selected ? (
                <ul className="text-list">
                  <li>Focused member: {selected.email}</li>
                  <li>Current backend state: {selected.status}</li>
                  <li>Most recent operation: {lastAction || "none"}</li>
                </ul>
              ) : (
                <p>Select or open a membership to inspect backend state.</p>
              )}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Membership list</CardTitle>
          <CardDescription>Current backend membership snapshots.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "email", label: "Email" },
              { key: "status", label: "Status" },
              { key: "activeUntil", label: "Active until" },
              { key: "graceUntil", label: "Grace until" },
            ]}
            rows={data.items.map((item) => ({
              id: item.email,
              activeUntil: item.active_until?.slice(0, 10) ?? "-",
              email: item.email,
              graceUntil: item.grace_until?.slice(0, 10) ?? "-",
              status: <Chip tone={item.status === "Active" ? "success" : item.status === "Grace" ? "warning" : "danger"}>{item.status}</Chip>,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
