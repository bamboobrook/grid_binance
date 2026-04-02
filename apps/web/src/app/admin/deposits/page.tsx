import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, FormStack } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminDepositsData } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ result?: string; tx?: string }>;
};

export default async function AdminDepositsPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const result = typeof params.result === "string" ? params.result : "";
  const tx = typeof params.tx === "string" ? params.tx : "";
  const data = await getAdminDepositsData();

  return (
    <>
      {result ? (
        <StatusBanner description={"Deposit result: " + result + (tx ? " | " + tx : "")} title="Deposit case updated" tone="success" />
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
                action:
                  item.status === "manual_review_required" ? (
                    <div className="content-grid">
                      <FormStack action="/api/admin/deposits" method="post">
                        <input name="txHash" type="hidden" value={item.tx_hash} />
                        <input name="chain" type="hidden" value={item.chain} />
                        <input name="decision" type="hidden" value="reject" />
                        <Button type="submit">{"Reject " + item.tx_hash}</Button>
                      </FormStack>
                      {item.order_id ? (
                        <FormStack action="/api/admin/deposits" method="post">
                          <input name="txHash" type="hidden" value={item.tx_hash} />
                          <input name="chain" type="hidden" value={item.chain} />
                          <input name="decision" type="hidden" value="credit_membership" />
                          <input name="orderId" type="hidden" value={String(item.order_id)} />
                          <Button type="submit">{"Credit " + item.tx_hash + " to membership"}</Button>
                        </FormStack>
                      ) : (
                        <span>-</span>
                      )}
                    </div>
                  ) : (
                    item.status
                  ),
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
            <CardDescription>Operator can see which order a credit action will target.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {data.abnormal_deposits.map((item) => (
                <li key={item.tx_hash}>
                  {item.tx_hash}: {item.order_id ? `order ${item.order_id}` : "no linked order"}
                </li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
