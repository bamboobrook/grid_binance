import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input, Select, Textarea } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminDepositsData, type AdminDepositView, type AdminDepositsResponse } from "../../../lib/api/admin-product-state";

const MANUAL_CREDIT_CONFIRMATION = "MANUAL_CREDIT_MEMBERSHIP";
const REVIEW_REASONS_REQUIRING_ORDER_SELECTION = new Set(["ambiguous_match", "order_not_found"]);

type PageProps = {
  searchParams?: Promise<{ error?: string; result?: string; tx?: string }>;
};

function manualCreditCandidateOrders(item: AdminDepositView, orders: AdminDepositsResponse["orders"]) {
  return orders
    .filter((order) => {
      if (order.status === "paid" || order.chain !== item.chain) {
        return false;
      }

      switch (item.review_reason) {
        case "ambiguous_match":
          return order.asset === item.asset && order.amount === item.amount && order.address === item.address;
        case "order_not_found":
          return order.asset === item.asset && order.amount === item.amount && order.address === item.address;
        default:
          return item.order_id === order.order_id;
      }
    })
    .sort((left, right) => {
      const leftScore = Number(left.amount === item.amount) + Number(left.order_id === item.order_id);
      const rightScore = Number(right.amount === item.amount) + Number(right.order_id === item.order_id);
      return rightScore - leftScore || left.order_id - right.order_id;
    });
}

function targetOrderLabel(order: AdminDepositsResponse["orders"][number]) {
  return `#${order.order_id} | ${order.email} | ${order.amount} ${order.asset} | ${order.status}`;
}

function renderManualActions(item: AdminDepositView, orders: AdminDepositsResponse["orders"]) {
  const candidateOrders = manualCreditCandidateOrders(item, orders);
  const defaultOrderId = item.order_id ?? candidateOrders[0]?.order_id ?? null;
  const requiresOrderSelection = REVIEW_REASONS_REQUIRING_ORDER_SELECTION.has(item.review_reason ?? "");
  const canSubmitCredit = requiresOrderSelection ? candidateOrders.length > 0 : Boolean(defaultOrderId);

  return (
    <div className="content-grid">
      <FormStack action="/api/admin/deposits" method="post">
        <input name="txHash" type="hidden" value={item.tx_hash} />
        <input name="chain" type="hidden" value={item.chain} />
        <input name="decision" type="hidden" value="reject" />
        <Button type="submit">{"Reject " + item.tx_hash}</Button>
      </FormStack>
      <FormStack action="/api/admin/deposits" method="post">
        <input name="txHash" type="hidden" value={item.tx_hash} />
        <input name="chain" type="hidden" value={item.chain} />
        <input name="decision" type="hidden" value="credit_membership" />
        {!requiresOrderSelection && defaultOrderId ? <input name="orderId" type="hidden" value={String(defaultOrderId)} /> : null}
        {requiresOrderSelection ? (
          <Field
            hint={
              candidateOrders.length > 0
                ? "Choose an eligible candidate order that matches this deposit context."
                : "No eligible candidate orders found for this deposit."
            }
            label={`Target order for ${item.tx_hash}`}
          >
            <Select defaultValue={defaultOrderId ? String(defaultOrderId) : ""} name="suggestedOrderId">
              <option value="">Select order</option>
              {candidateOrders.map((order) => (
                <option key={order.order_id} value={String(order.order_id)}>
                  {targetOrderLabel(order)}
                </option>
              ))}
            </Select>
          </Field>
        ) : (
          <p>{defaultOrderId ? `Target order: ${defaultOrderId}` : "No linked order available for manual credit."}</p>
        )}
        <Field hint={`Type ${MANUAL_CREDIT_CONFIRMATION} to confirm the manual membership credit.`} label={`Confirmation for ${item.tx_hash}`}>
          <Input autoComplete="off" name="confirmation" />
        </Field>
        <Field label={`Justification for ${item.tx_hash}`}>
          <Textarea name="justification" rows={3} />
        </Field>
        <Button disabled={!canSubmitCredit} type="submit">{"Credit " + item.tx_hash + " to membership"}</Button>
      </FormStack>
    </div>
  );
}

export default async function AdminDepositsPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const error = typeof params.error === "string" ? params.error : "";
  const result = typeof params.result === "string" ? params.result : "";
  const tx = typeof params.tx === "string" ? params.tx : "";
  const data = await getAdminDepositsData();

  return (
    <>
      {result ? (
        <StatusBanner description={"Deposit result: " + result + (tx ? " | " + tx : "")} title="Deposit case updated" tone="success" />
      ) : null}
      {error ? (
        <StatusBanner description={error + (tx ? " | " + tx : "")} title="Deposit action failed" tone="warning" />
      ) : null}
      <AppShellSection
        description="Manual review decisions are read from and written to backend deposit workflows."
        eyebrow="Deposit review"
        title="Abnormal Deposit Handling"
      >
        <Card>
          <CardHeader>
            <CardTitle>Deposit exception queue</CardTitle>
            <CardDescription>Manual credit and rejection both route through backend review decisions.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "tx", label: "Tx hash" },
                { key: "chain", label: "Chain" },
                { key: "reason", label: "Reason" },
                { key: "status", label: "Status" },
                { key: "action", label: "Actions" },
              ]}
              rows={data.abnormal_deposits.map((item) => ({
                id: item.tx_hash,
                action: item.status === "manual_review_required" ? renderManualActions(item, data.orders) : item.status,
                chain: item.chain,
                reason: item.review_reason ?? "-",
                status: item.status,
                tx: item.tx_hash,
              }))}
            />
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Manual credit target order</CardTitle>
            <CardDescription>Operator can confirm the current target and see only eligible candidate orders for unresolved review cases.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {data.abnormal_deposits.map((item) => {
                const candidateOrders = manualCreditCandidateOrders(item, data.orders);
                const needsSelection = REVIEW_REASONS_REQUIRING_ORDER_SELECTION.has(item.review_reason ?? "");
                return (
                  <li key={item.tx_hash}>
                    {item.tx_hash}: {item.order_id ? `order ${item.order_id}` : "no linked order"}
                    {needsSelection && candidateOrders.length > 0
                      ? ` | eligible ${candidateOrders.map((order) => order.order_id).join(", ")}`
                      : ""}
                  </li>
                );
              })}
            </ul>
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
