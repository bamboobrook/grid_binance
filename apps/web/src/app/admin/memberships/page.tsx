import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import {
  getAdminMembershipPlansData,
  getAdminMembershipsData,
  getCurrentAdminProfile,
} from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ action?: string; email?: string; planSaved?: string }>;
};

export default async function AdminMembershipsPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const selectedEmail = typeof params.email === "string" ? params.email : "";
  const lastAction = typeof params.action === "string" ? params.action : "";
  const planSaved = typeof params.planSaved === "string" ? params.planSaved : "";
  const [profile, memberships, plans] = await Promise.all([
    getCurrentAdminProfile(),
    getAdminMembershipsData(),
    getAdminMembershipPlansData(),
  ]);
  const selected = memberships.items.find((item) => item.email === selectedEmail) ?? null;
  const canManage = profile.admin_permissions?.can_manage_memberships ?? false;
  const canManagePlans = profile.admin_permissions?.can_manage_plans ?? false;
  const monthly = plans.plans.find((plan) => plan.code === "monthly") ?? plans.plans[0] ?? null;
  const priceFor = (chain: string, asset: string) => monthly?.prices.find((price) => price.chain === chain && price.asset === asset)?.amount ?? "20.00";

  return (
    <>
      {selected && lastAction ? (
        <StatusBanner
          description={`Target: ${selected.email} | Status: ${selected.status} | Last action: ${lastAction}`}
          title="Membership updated"
          tone="success"
        />
      ) : null}
      {planSaved ? <StatusBanner description={`Updated pricing plan code: ${planSaved}`} title="Plan pricing saved" tone="success" /> : null}
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
                  <Field label="BSC / USDT price">
                    <Input defaultValue={priceFor("BSC", "USDT")} name="bscUsdtPrice" />
                  </Field>
                  <Field label="ETH / USDT price">
                    <Input defaultValue={priceFor("ETH", "USDT")} name="ethUsdtPrice" />
                  </Field>
                  <Field label="SOL / USDC price">
                    <Input defaultValue={priceFor("SOL", "USDC")} name="solUsdcPrice" />
                  </Field>
                  <Button type="submit">Save plan pricing</Button>
                </FormStack>
              ) : (
                <p>super_admin required for plan and pricing changes.</p>
              )}
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Manual membership controls</CardTitle>
              <CardDescription>Operate directly against backend membership actions.</CardDescription>
            </CardHeader>
            <CardBody>
              {canManage ? (
                <>
                  <FormStack action="/api/admin/memberships" method="post">
                    <Field label="Member email">
                      <Input defaultValue={selectedEmail} name="email" placeholder="member@example.com" />
                    </Field>
                    <Field label="Membership duration days">
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
                </>
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
            rows={memberships.items.map((item) => ({
              id: item.email,
              activeUntil: item.active_until?.slice(0, 10) ?? "-",
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
