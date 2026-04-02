import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { DialogFrame } from "../../../components/ui/dialog";
import { Button, FormStack } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getCurrentAdminProductState } from "../../../lib/api/admin-product-state";

export default async function AdminDepositsPage() {
  const state = await getCurrentAdminProductState();

  return (
    <>
      {state.flash.deposits ? (
        <StatusBanner description={state.flash.deposits.description} title={state.flash.deposits.title} tone={state.flash.deposits.tone} />
      ) : null}
      <AppShellSection
        description="Resolve wrong-token, underpayment, overpayment, and abnormal transfer cases without silently crediting memberships."
        eyebrow="Deposit review"
        title="Abnormal Deposit Handling"
      >
        <Card>
          <CardHeader>
            <CardTitle>Abnormal deposit queue</CardTitle>
            <CardDescription>Open cases remain blocked until an operator makes an explicit decision.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "order", label: "Order" },
                { key: "issue", label: "Issue" },
                { key: "user", label: "User" },
                { key: "amount", label: "Amount", align: "right" },
                { key: "state", label: "State" },
                { key: "action", label: "Action", align: "right" },
              ]}
              rows={state.deposits.map((item) => ({
                id: item.id,
                action:
                  item.order === "ORD-4201" ? (
                    <FormStack action="/api/admin/deposits" method="post">
                      <input name="depositId" type="hidden" value={item.id} />
                      <Button type="submit">Resolve ORD-4201 as refunded</Button>
                    </FormStack>
                  ) : (
                    <Chip tone={item.state === "open" ? "warning" : "success"}>{item.state}</Chip>
                  ),
                amount: `${item.amount} ${item.token}`,
                issue: item.issue,
                order: item.order,
                state: <Chip tone={item.state === "open" ? "danger" : "success"}>{item.state}</Chip>,
                user: item.user,
              }))}
            />
          </CardBody>
        </Card>
      </AppShellSection>
      <div className="content-grid content-grid--split">
        <DialogFrame
          description="Overpayment, underpayment, wrong token, and abnormal transfer must be held for manual handling. No auto-credit path exists here."
          title="Manual handling rule"
          tone="danger"
        >
          <ul className="text-list">
            <li>Open cases: {state.deposits.filter((item) => item.state === "open").length}</li>
            <li>Refunded cases: {state.deposits.filter((item) => item.state === "refunded").length}</li>
          </ul>
        </DialogFrame>
        <Card>
          <CardHeader>
            <CardTitle>Operator notes</CardTitle>
            <CardDescription>Use these notes before messaging the user or treasury team.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {state.deposits.map((item) => (
                <li key={item.id}>
                  {item.order}: {item.note}
                </li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}
