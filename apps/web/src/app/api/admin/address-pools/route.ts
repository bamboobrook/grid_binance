import { appendAuditRecord, updateAdminProductState } from "../../../../lib/api/admin-product-state";

import { readField, readSessionToken, redirectTo } from "../_shared";

const chainNames = {
  bsc: "BSC",
  ethereum: "Ethereum",
  solana: "Solana",
} as const;

export async function POST(request: Request) {
  const formData = await request.formData();
  const chain = readField(formData, "chain") as keyof typeof chainNames;
  const expandBy = Number(readField(formData, "expandBy") || "0");
  const sessionToken = readSessionToken(request);

  updateAdminProductState(sessionToken, (state) => {
    const pool = state.addressPools.find((item) => item.chain === chain);
    if (!pool || Number.isNaN(expandBy) || expandBy <= 0) {
      state.flash.addressPools = {
        description: "Choose a valid chain and expansion amount.",
        title: "Pool update failed",
        tone: "danger",
      };
      return;
    }

    pool.total += expandBy;
    state.flash.addressPools = {
      description: `${chainNames[chain]} now has ${pool.total} total addresses with ${pool.total - pool.locked} free slots.`,
      title: "Pool capacity updated",
      tone: "success",
    };
    appendAuditRecord(state, {
      action: "pool.expand",
      actor: "Operator Nova",
      domain: "pool",
      summary: `Expanded ${chainNames[chain]} pool by ${expandBy} addresses.`,
      target: chainNames[chain],
    });
  });

  return redirectTo(request, "/admin/address-pools");
}
