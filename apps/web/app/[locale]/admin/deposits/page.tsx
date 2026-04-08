import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, Field, FormStack, Input, Select, Textarea } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import { getAdminDepositsData, type AdminDepositView, type AdminDepositsResponse } from "@/lib/api/admin-product-state";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE, type UiLanguage } from "@/lib/ui/preferences";

const MANUAL_CREDIT_CONFIRMATION = "confirm manual credit";
const REVIEW_REASONS_REQUIRING_ORDER_SELECTION = new Set(["ambiguous_match", "order_not_found"]);

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{ error?: string; result?: string; tx?: string }>;
};

function reviewReasonLabel(lang: UiLanguage, reason: string | null) {
  switch (reason) {
    case "ambiguous_match":
      return pickText(lang, "匹配不唯一", "Ambiguous Match");
    case "order_not_found":
      return pickText(lang, "未找到订单", "Order Not Found");
    case "amount_mismatch":
      return pickText(lang, "金额不匹配", "Amount Mismatch");
    case "asset_mismatch":
      return pickText(lang, "币种不匹配", "Asset Mismatch");
    default:
      return reason ?? pickText(lang, "未标记", "Unspecified");
  }
}

function depositStatusLabel(lang: UiLanguage, status: string) {
  switch (status) {
    case "manual_review_required":
      return pickText(lang, "待人工复核", "Manual Review Required");
    case "credited":
      return pickText(lang, "已人工入账", "Credited");
    case "rejected":
      return pickText(lang, "已驳回", "Rejected");
    default:
      return status;
  }
}

function manualCreditCandidateOrders(item: AdminDepositView, orders: AdminDepositsResponse["orders"]) {
  return orders
    .filter((order) => {
      if (order.status === "paid" || item.chain !== order.chain) {
        return false;
      }

      switch (item.review_reason) {
        case "ambiguous_match":
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

function targetOrderLabel(lang: UiLanguage, order: AdminDepositsResponse["orders"][number]) {
  return pickText(lang, "订单 #" + String(order.order_id) + "，" + order.email + "，" + order.amount + " " + order.asset + "，状态 " + order.status, "Order #" + String(order.order_id) + ", " + order.email + ", " + order.amount + " " + order.asset + ", status " + order.status);
}

function renderManualActions(lang: UiLanguage, item: AdminDepositView, orders: AdminDepositsResponse["orders"]) {
  const candidateOrders = manualCreditCandidateOrders(item, orders);
  const defaultOrderId = item.order_id ?? candidateOrders[0]?.order_id ?? null;
  const requiresOrderSelection = REVIEW_REASONS_REQUIRING_ORDER_SELECTION.has(item.review_reason ?? "");
  const canSubmitCredit = requiresOrderSelection ? candidateOrders.length > 0 : Boolean(defaultOrderId);

  return (
    <div className="grid grid-cols-1 gap-4">
      <FormStack action="/api/admin/deposits" method="post">
        <input name="txHash" type="hidden" value={item.tx_hash} />
        <input name="chain" type="hidden" value={item.chain} />
        <input name="decision" type="hidden" value="reject" />
        <Button type="submit">{pickText(lang, "驳回充值", "Reject Deposit")}</Button>
      </FormStack>
      <FormStack action="/api/admin/deposits" method="post">
        <input name="txHash" type="hidden" value={item.tx_hash} />
        <input name="chain" type="hidden" value={item.chain} />
        <input name="decision" type="hidden" value="credit_membership" />
        {requiresOrderSelection === false && defaultOrderId ? <input name="orderId" type="hidden" value={String(defaultOrderId)} /> : null}
        {requiresOrderSelection ? (
          <Field
            hint={candidateOrders.length > 0 ? pickText(lang, "请选择与这笔充值上下文一致的订单。", "Choose the eligible order that matches this deposit context.") : pickText(lang, "当前没有可用于人工入账的候选订单。", "No eligible candidate orders were found for this deposit.")}
            label={pickText(lang, "目标订单", "Target Order")}
          >
            <Select defaultValue={defaultOrderId ? String(defaultOrderId) : ""} name="suggestedOrderId">
              <option value="">{pickText(lang, "请选择订单", "Select Order")}</option>
              {candidateOrders.map((order) => (
                <option key={order.order_id} value={String(order.order_id)}>
                  {targetOrderLabel(lang, order)}
                </option>
              ))}
            </Select>
          </Field>
        ) : (
          <p>{defaultOrderId ? pickText(lang, "目标订单：" + String(defaultOrderId), "Target order: " + String(defaultOrderId)) : pickText(lang, "当前没有关联订单，无法直接入账。", "No linked order is available for manual credit.")}</p>
        )}
        <Field hint={pickText(lang, "请输入确认短语“" + MANUAL_CREDIT_CONFIRMATION + "”，表示你已经核对链路、金额和归属。", "Type the confirmation phrase “" + MANUAL_CREDIT_CONFIRMATION + "” to confirm chain, amount, and ownership review.")} label={pickText(lang, "确认短语", "Confirmation Phrase")}>
          <Input autoComplete="off" name="confirmation" />
        </Field>
        <Field label={pickText(lang, "复核说明", "Review Notes")}>
          <Textarea name="justification" rows={3} />
        </Field>
        <Button disabled={canSubmitCredit === false} type="submit">{pickText(lang, "人工入账", "Manual Credit")}</Button>
      </FormStack>
    </div>
  );
}

export default async function AdminDepositsPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const query = (await searchParams) ?? {};
  const [cookieStore, data] = await Promise.all([cookies(), getAdminDepositsData()]);
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const error = typeof query.error === "string" ? query.error : "";
  const result = typeof query.result === "string" ? query.result : "";
  const tx = typeof query.tx === "string" ? query.tx : "";
  const manualQueue = data.abnormal_deposits.filter((item) => item.status === "manual_review_required").length;

  return (
    <>
      {result ? <StatusBanner description={pickText(lang, "处理结果：" + result + (tx ? "，交易 " + tx : ""), "Result: " + result + (tx ? ", tx " + tx : ""))} title={pickText(lang, "充值案件已更新", "Deposit Case Updated")} /> : null}
      {error ? <StatusBanner description={tx ? error + " (" + tx + ")" : error} title={pickText(lang, "充值动作失败", "Deposit Action Failed")} /> : null}
      <AppShellSection
        description={pickText(lang, "值班席位直接处理充值异常、订单匹配和人工入账说明，确保术语与风险提示都对人可读。", "The desk handles deposit exceptions, order matching, and manual credit notes with human-readable risk prompts.")}
        eyebrow={pickText(lang, "充值审核", "Deposit Review")}
        title={pickText(lang, "异常充值处理", "Abnormal Deposit Handling")}
      >
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "异常队列", "Exception Queue")}</CardTitle>
              <CardDescription>{pickText(lang, "当前有 " + String(manualQueue) + " 笔待人工复核。", String(manualQueue) + " deposits currently require manual review.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <div className="overflow-x-auto whitespace-nowrap min-w-full pb-4 rounded-lg">
                <DataTable
                columns={[
                  { key: "tx", label: pickText(lang, "交易哈希", "Tx Hash") },
                  { key: "chain", label: pickText(lang, "链路", "Chain") },
                  { key: "reason", label: pickText(lang, "复核原因", "Review Reason") },
                  { key: "status", label: pickText(lang, "状态", "Status") },
                  { key: "action", label: pickText(lang, "动作", "Actions") },
                ]}
                rows={data.abnormal_deposits.map((item) => ({
                  id: item.tx_hash,
                  action: item.status === "manual_review_required" ? renderManualActions(lang, item, data.orders) : depositStatusLabel(lang, item.status),
                  chain: item.chain,
                  reason: reviewReasonLabel(lang, item.review_reason),
                  status: depositStatusLabel(lang, item.status),
                  tx: item.tx_hash,
                }))}
              />
              </div>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "复核说明", "Desk Notes")}</CardTitle>
              <CardDescription>{pickText(lang, "确认短语仍保留给后端校验，但页面上用人类可读的操作说明展示。", "The confirmation phrase is still kept for backend validation, but the UI explains it in human terms.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>{pickText(lang, "先确认链路、币种、金额和目标订单，再执行人工入账。", "Confirm chain, asset, amount, and target order before manual credit.")}</li>
                <li>{pickText(lang, "匹配不唯一或未找到订单时，必须显式选择目标订单。", "When the match is ambiguous or missing, choose the target order explicitly.")}</li>
                <li>{pickText(lang, "复核说明会进入审计，避免留下只有内部人才懂的缩写。", "Review notes enter audit trails, so avoid unexplained internal shorthand.")}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "目标订单快照", "Target Order Snapshot")}</CardTitle>
          <CardDescription>{pickText(lang, "供值班席位快速判断当前充值对应的候选订单。", "Helps the desk quickly verify the candidate orders tied to each deposit case.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <ul className="text-list">
            {data.abnormal_deposits.map((item) => {
              const candidateOrders = manualCreditCandidateOrders(item, data.orders);
              const needsSelection = REVIEW_REASONS_REQUIRING_ORDER_SELECTION.has(item.review_reason ?? "");
              return (
                <li key={item.tx_hash}>
                  {pickText(lang, item.tx_hash + "：" + reviewReasonLabel(lang, item.review_reason) + "；" + (item.order_id ? "当前订单 " + String(item.order_id) : "暂无关联订单") + (needsSelection && candidateOrders.length > 0 ? "；候选 " + candidateOrders.map((order) => order.order_id).join("、") : ""), item.tx_hash + ": " + reviewReasonLabel(lang, item.review_reason) + "; " + (item.order_id ? "current order " + String(item.order_id) : "no linked order") + (needsSelection && candidateOrders.length > 0 ? "; eligible " + candidateOrders.map((order) => order.order_id).join(", ") : ""))}
                </li>
              );
            })}
          </ul>
        </CardBody>
      </Card>
    </>
  );
}
