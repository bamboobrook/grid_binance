import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, Field, FormStack, Input, Select } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import {
  SUPPORTED_PAYMENT_ASSETS,
  SUPPORTED_PAYMENT_CHAINS,
  getAdminSweepsData,
  getCurrentAdminProfile,
} from "@/lib/api/admin-product-state";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";
import { formatTaipeiDateTime } from "@/lib/ui/time";

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{ asset?: string; chain?: string; submitted?: string; treasury?: string }>;
};

export default async function AdminSweepsPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const query = (await searchParams) ?? {};
  const submitted = query.submitted === "1";
  const treasury = typeof query.treasury === "string" ? query.treasury : "";
  const selectedChain = typeof query.chain === "string" ? query.chain : "BSC";
  const selectedAsset = typeof query.asset === "string" ? query.asset : "USDT";
  const [cookieStore, profile, data] = await Promise.all([cookies(), getCurrentAdminProfile(), getAdminSweepsData()]);
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const canManageSweeps = profile.admin_permissions?.can_manage_sweeps ?? false;

  return (
    <>
      {submitted ? (
        <StatusBanner
                tone="info"
                lang={lang}
          description={pickText(lang, "最近归集目标：" + (treasury || "-") + "，链路资产：" + selectedChain + " / " + selectedAsset, "Latest sweep destination: " + (treasury || "-") + ", route " + selectedChain + " / " + selectedAsset)}
          title={pickText(lang, "归集请求已提交", "Sweep Request Submitted")}
         
        />
      ) : null}
      <AppShellSection
        description={pickText(lang, "值班席位处理稳定币归集申请、金库地址与失败回执。", "The desk handles stablecoin sweep requests, treasury destinations, and failure receipts.")}
        eyebrow={pickText(lang, "归集审批", "Sweep Approval")}
        title={pickText(lang, "归集操作", "Sweep Operations")}
      >
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "提交归集", "Submit Sweep")}</CardTitle>
              <CardDescription>{pickText(lang, "从源地址向金库地址发起管理端归集。", "Initiate treasury-bound sweep jobs from source addresses.")}</CardDescription>
            </CardHeader>
            <CardBody>
              {canManageSweeps ? (
                <FormStack action="/api/admin/sweeps" method="post">
                  <Field label={pickText(lang, "链路", "Chain")}>
                    <Select defaultValue={selectedChain} name="chain">
                      {SUPPORTED_PAYMENT_CHAINS.map((chain) => (
                        <option key={chain} value={chain}>{chain}</option>
                      ))}
                    </Select>
                  </Field>
                  <Field label={pickText(lang, "资产", "Asset")}>
                    <Select defaultValue={selectedAsset} name="asset">
                      {SUPPORTED_PAYMENT_ASSETS.map((asset) => (
                        <option key={asset} value={asset}>{asset}</option>
                      ))}
                    </Select>
                  </Field>
                  <Field label={pickText(lang, "金库地址", "Treasury Address")}>
                    <Input name="treasuryAddress" placeholder="treasury-bsc-main" />
                  </Field>
                  <Field label={pickText(lang, "源地址", "Source Address")}>
                    <Input name="fromAddress" placeholder="bsc-addr-2" />
                  </Field>
                  <Field label={pickText(lang, "归集金额", "Sweep Amount")}>
                    <Input name="amount" placeholder="18.50000000" />
                  </Field>
                  <Button type="submit">{pickText(lang, "提交归集", "Submit Sweep")}</Button>
                </FormStack>
              ) : (
                <p>{pickText(lang, "需要 super_admin 才能执行归集操作。", "A super_admin session is required for sweep operations.")}</p>
              )}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "归集任务队列", "Queued Treasury Jobs")}</CardTitle>
          <CardDescription>{pickText(lang, "逐行暴露金库地址、生命周期和失败原因。", "Treasury destination, lifecycle, and failure detail stay visible row by row.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "id", label: pickText(lang, "任务", "Job") },
              { key: "chain", label: pickText(lang, "链路资产", "Route") },
              { key: "treasury", label: pickText(lang, "金库地址", "Treasury") },
              { key: "lifecycle", label: pickText(lang, "生命周期", "Lifecycle") },
              { key: "status", label: pickText(lang, "状态", "Status") },
            ]}
            rows={data.jobs.map((item) => ({
              id: String(item.sweep_job_id),
              chain: item.chain + " / " + item.asset,
              lifecycle: item.failed_at
                ? pickText(lang, "失败时间 " + formatTaipeiDateTime(item.failed_at, lang) + "，原因：" + (item.last_error ?? "无错误详情"), "Failed at " + formatTaipeiDateTime(item.failed_at, lang) + ", error: " + (item.last_error ?? "no error detail"))
                : item.submitted_at
                  ? pickText(lang, "提交时间 " + formatTaipeiDateTime(item.submitted_at, lang) + "，转账数 " + String(item.transfer_count), "Submitted at " + formatTaipeiDateTime(item.submitted_at, lang) + ", transfers " + String(item.transfer_count))
                  : pickText(lang, String(item.transfer_count) + " 笔待转账", String(item.transfer_count) + " pending transfers"),
              status: item.status,
              treasury: item.treasury_address,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
