import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable, type DataTableColumn } from "../../../components/ui/table";
import {
  getAdminMembershipPlansData,
  getAdminMembershipsData,
  getCurrentAdminProfile,
} from "../../../lib/api/admin-product-state";

const SUPPORTED_CHAINS = ["ETH", "BSC", "SOL"] as const;
const SUPPORTED_ASSETS = ["USDT", "USDC"] as const;
const SUPPORTED_PRICE_MATRIX = SUPPORTED_CHAINS.flatMap((chain) =>
  SUPPORTED_ASSETS.map((asset) => ({ chain, asset, fieldName: `price:${chain}:${asset}` })),
);

type PageProps = {
  searchParams?: Promise<{ action?: string; planError?: string; planSaved?: string; target?: string }>;
};

export default async function AdminMembershipsPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const targetEmail = typeof params.target === "string" ? params.target : "";
  const lastAction = typeof params.action === "string" ? params.action : "";
  const planSaved = typeof params.planSaved === "string" ? params.planSaved : "";
  const planError = typeof params.planError === "string" ? params.planError : "";
  const [profile, memberships, plans] = await Promise.all([
    getCurrentAdminProfile(),
    getAdminMembershipsData(),
    getAdminMembershipPlansData(),
  ]);
  const updatedMembership = memberships.items.find((item) => item.email === targetEmail) ?? null;
  const canManage = profile.admin_permissions?.can_manage_memberships ?? false;
  const canManagePlans = profile.admin_permissions?.can_manage_plans ?? false;
  const monthly = plans.plans.find((plan) => plan.code === "monthly") ?? plans.plans[0] ?? null;
  const priceFor = (chain: string, asset: string) => monthly?.prices.find((price) => price.chain === chain && price.asset === asset)?.amount ?? "";
  const membershipColumns: DataTableColumn[] = [
    { key: "email", label: "Email" },
    { key: "status", label: "Status" },
    { key: "activeUntil", label: "Active until" },
    { key: "graceUntil", label: "Grace until" },
  ];

  if (canManage) {
    membershipColumns.push({ key: "actions", label: "Actions" });
  }

  return (
    <>
      {updatedMembership && lastAction ? (
        <StatusBanner
          description={`Target: ${updatedMembership.email} | Status: ${updatedMembership.status} | Last action: ${lastAction}`}
          title="Membership updated"
          tone="success"
        />
      ) : null}
      {planSaved ? <StatusBanner description={`Updated pricing plan code: ${planSaved}`} title="Plan pricing saved" tone="success" /> : null}
      {planError ? <StatusBanner description={planError} title="Plan pricing not saved" tone="warning" /> : null}
      <AppShellSection
        description="Backend-backed membership operations and plan pricing controls."
        eyebrow="Membership operations"
        title="Membership Operations"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Plan & pricing management</CardTitle>
              <CardDescription>Monthly, quarterly, and yearly pricing are managed here.</CardDescription>
            </CardHeader>
            <CardBody>
              {canManagePlans ? (
                <FormStack action="/api/admin/memberships" method="post">
                  <input name="intent" type="hidden" value="save-plan" />
                  <Field label="Plan code">
                    <Input defaultValue={monthly?.code ?? "monthly"} name="code" />
                  </Field>
                  <Field label="Display name">
                    <Input defaultValue={monthly?.name ?? "Monthly"} name="name" />
                  </Field>
                  <Field label="Plan duration days">
                    <Input defaultValue={String(monthly?.duration_days ?? 30)} inputMode="numeric" name="durationDays" />
                  </Field>
                  {SUPPORTED_PRICE_MATRIX.map(({ chain, asset, fieldName }) => (
                    <Field key={fieldName} label={`${chain} / ${asset} price`}>
                      <Input defaultValue={priceFor(chain, asset)} name={fieldName} />
                    </Field>
                  ))}
                  <Button type="submit">Save plan pricing</Button>
                </FormStack>
              ) : (
                <p>super_admin required for plan and pricing changes.</p>
              )}
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Open membership</CardTitle>
              <CardDescription>Create or reopen a membership without relying on a pre-selected row.</CardDescription>
            </CardHeader>
            <CardBody>
              {canManage ? (
                <FormStack action="/api/admin/memberships" method="post">
                  <Field label="Member email">
                    <Input name="email" placeholder="member@example.com" />
                  </Field>
                  <Field label="Membership duration days">
                    <Input defaultValue="30" inputMode="numeric" name="durationDays" />
                  </Field>
                  <input name="action" type="hidden" value="open" />
                  <Button type="submit">Open membership</Button>
                </FormStack>
              ) : (
                <p>Membership lifecycle changes are locked to super_admin.</p>
              )}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Current plan pricing</CardTitle>
          <CardDescription>Backend plan pricing snapshots.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "code", label: "Plan" },
              { key: "duration", label: "Duration" },
              { key: "prices", label: "Prices" },
            ]}
            rows={plans.plans.map((plan) => ({
              id: plan.code,
              code: plan.code,
              duration: `${plan.duration_days} days`,
              prices: plan.prices.map((price) => `${price.chain} ${price.asset} ${price.amount}`).join(" | "),
            }))}
          />
        </CardBody>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Membership list</CardTitle>
          <CardDescription>Current backend membership snapshots with row-level controls.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={membershipColumns}
            rows={memberships.items.map((item) => ({
              id: item.email,
              activeUntil: item.active_until?.slice(0, 10) ?? "-",
              actions: canManage ? (
                <div>
                  <FormStack action="/api/admin/memberships" method="post">
                    <input name="email" type="hidden" value={item.email} />
                    {item.email === targetEmail ? (
                      <Field label="Membership duration days">
                        <Input defaultValue="15" inputMode="numeric" name="durationDays" />
                      </Field>
                    ) : (
                      <Field label="Extend duration days">
                        <Input defaultValue="15" inputMode="numeric" name="durationDays" />
                      </Field>
                    )}
                    <input name="action" type="hidden" value="extend" />
                    <Button type="submit">Extend membership</Button>
                  </FormStack>
                  {item.status === "Frozen" ? (
                    <FormStack action="/api/admin/memberships" method="post">
                      <input name="email" type="hidden" value={item.email} />
                      <input name="action" type="hidden" value="unfreeze" />
                      <Button type="submit">Unfreeze membership</Button>
                    </FormStack>
                  ) : (
                    <FormStack action="/api/admin/memberships" method="post">
                      <input name="email" type="hidden" value={item.email} />
                      <input name="action" type="hidden" value="freeze" />
                      <Button type="submit">Freeze membership</Button>
                    </FormStack>
                  )}
                  <FormStack action="/api/admin/memberships" method="post">
                    <input name="email" type="hidden" value={item.email} />
                    <input name="action" type="hidden" value="revoke" />
                    <Button type="submit">Revoke membership</Button>
                  </FormStack>
                </div>
              ) : null,
              email: item.email,
              graceUntil: item.grace_until?.slice(0, 10) ?? "-",
              status: item.status,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
