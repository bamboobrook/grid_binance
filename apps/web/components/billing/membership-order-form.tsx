"use client";

import { useEffect, useMemo, useState } from "react";

import { Button, Field, FormStack, Select } from "@/components/ui/form";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type PriceOption = {
  amount: string;
  asset: string;
  chain: string;
};

type PlanOption = {
  code: string;
  name: string;
  prices: PriceOption[];
};

type MembershipOrderFormProps = {
  activeUntil?: string | null;
  lang: UiLanguage;
  initialChain?: string;
  initialPlanCode?: string;
  initialToken?: string;
  plans: PlanOption[];
};

export function MembershipOrderForm({
  activeUntil,
  lang,
  initialChain,
  initialPlanCode,
  initialToken,
  plans,
}: MembershipOrderFormProps) {
  const [selectedPlanCode, setSelectedPlanCode] = useState(initialPlanCode || plans[0]?.code || "");
  const selectedPlan = useMemo(
    () => plans.find((plan) => plan.code === selectedPlanCode) ?? plans[0] ?? null,
    [plans, selectedPlanCode],
  );
  const chainOptions = useMemo(
    () => uniqueValues((selectedPlan?.prices ?? []).map((price) => price.chain)),
    [selectedPlan],
  );
  const [selectedChain, setSelectedChain] = useState(initialChain || chainOptions[0] || "");
  const tokenOptions = useMemo(
    () => uniqueValues((selectedPlan?.prices ?? []).filter((price) => !selectedChain || price.chain === selectedChain).map((price) => price.asset)),
    [selectedPlan, selectedChain],
  );
  const [selectedToken, setSelectedToken] = useState(initialToken || tokenOptions[0] || "");

  useEffect(() => {
    if (chainOptions.length === 0) {
      if (selectedChain) {
        setSelectedChain("");
      }
      return;
    }
    if (!chainOptions.includes(selectedChain)) {
      setSelectedChain(chainOptions[0]);
    }
  }, [chainOptions, selectedChain]);

  useEffect(() => {
    if (tokenOptions.length === 0) {
      if (selectedToken) {
        setSelectedToken("");
      }
      return;
    }
    if (!tokenOptions.includes(selectedToken)) {
      setSelectedToken(tokenOptions[0]);
    }
  }, [selectedToken, tokenOptions]);

  const selectedPrice = useMemo(() => {
    if (!selectedPlan) {
      return null;
    }
    return (
      selectedPlan.prices.find((price) => price.chain === selectedChain && price.asset === selectedToken) ??
      selectedPlan.prices.find((price) => !selectedChain || price.chain === selectedChain) ??
      selectedPlan.prices[0] ??
      null
    );
  }, [selectedChain, selectedPlan, selectedToken]);

  return (
    <FormStack action="/api/user/billing" method="post" className="gap-4">
      <p>
        {pickText(lang, "下次续费时间", "Next renewal")}: {activeUntil?.slice(0, 10) ?? pickText(lang, "暂无", "Unavailable")}
      </p>
      <p>
        {pickText(lang, "当前选择价格", "Selected price")}: {describeSelectedPrice(lang, selectedPrice)}
      </p>
      <Field label={pickText(lang, "套餐", "Plan")}>
        <Select name="plan" value={selectedPlanCode} onChange={(event) => setSelectedPlanCode(event.target.value)}>
          {plans.length === 0 ? <option value="">{pickText(lang, "暂无可用套餐", "No plans available")}</option> : null}
          {plans.map((plan) => (
            <option key={plan.code} value={plan.code}>{labelForPlan(lang, plan.code, plan.name)}</option>
          ))}
        </Select>
      </Field>
      <Field label={pickText(lang, "链路", "Chain")}>
        <Select name="chain" value={selectedChain} onChange={(event) => setSelectedChain(event.target.value)}>
          {chainOptions.length === 0 ? <option value="">{pickText(lang, "暂无可用链路", "No chain available")}</option> : null}
          {chainOptions.map((chain) => (
            <option key={chain} value={chain}>{chain}</option>
          ))}
        </Select>
      </Field>
      <Field label={pickText(lang, "稳定币", "Token")}>
        <Select name="token" value={selectedToken} onChange={(event) => setSelectedToken(event.target.value)}>
          {tokenOptions.length === 0 ? <option value="">{pickText(lang, "暂无可用币种", "No token available")}</option> : null}
          {tokenOptions.map((token) => (
            <option key={token} value={token}>{token}</option>
          ))}
        </Select>
      </Field>
      <Button type="submit">{pickText(lang, "创建支付订单", "Create payment order")}</Button>
    </FormStack>
  );
}

export function describePlanSummary(lang: UiLanguage, plan: PlanOption) {
  return `${labelForPlan(lang, plan.code, plan.name)} ${firstUsdAmount(plan)}`;
}

function describeSelectedPrice(lang: UiLanguage, price: PriceOption | null) {
  if (!price) {
    return pickText(lang, "暂无可用价格", "No price available");
  }
  return `${price.amount} USD · ${price.chain} · ${price.asset}`;
}

function firstUsdAmount(plan: PlanOption) {
  return `${plan.prices[0]?.amount ?? "0"} USD`;
}

function labelForPlan(lang: UiLanguage, code: string, fallback: string) {
  switch (code.trim().toLowerCase()) {
    case "monthly":
      return pickText(lang, "按月支付", "Pay monthly");
    case "quarterly":
      return pickText(lang, "按季度支付", "Pay quarterly");
    case "yearly":
      return pickText(lang, "按年支付", "Pay yearly");
    default:
      return fallback;
  }
}

function uniqueValues(values: string[]) {
  return Array.from(new Set(values));
}
