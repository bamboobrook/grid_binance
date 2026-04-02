import "server-only";

import {
  adminAddressPoolsSnapshot,
  adminAuditSnapshot,
  adminBillingSnapshot,
  adminDashboardSnapshot,
  adminTemplatesSnapshot,
  adminUsersSnapshot,
  analyticsSnapshot,
  billingSnapshot,
  buildAdminShellSnapshot,
  buildUserShellSnapshot,
  exchangeSnapshot,
  membershipSnapshot,
  notificationsSnapshot,
  publicAuthSnapshots,
  publicShellSnapshot,
  securitySnapshot,
  strategiesSnapshot,
  strategyDetailSnapshots,
  type AdminShellSnapshot,
  type PublicShellSnapshot,
  type UserShellSnapshot,
  userDashboardSnapshot,
} from "./mock-data";

function clone<T>(value: T): T {
  return structuredClone(value);
}

export async function getPublicShellSnapshot(): Promise<PublicShellSnapshot> {
  return clone(publicShellSnapshot);
}

export async function getPublicAuthSnapshot(mode: "login" | "register") {
  return clone(publicAuthSnapshots[mode]);
}

export async function getUserShellSnapshot(activeHref: string): Promise<UserShellSnapshot> {
  return clone(buildUserShellSnapshot(activeHref));
}

export async function getAdminShellSnapshot(activeHref: string): Promise<AdminShellSnapshot> {
  return clone(buildAdminShellSnapshot(activeHref));
}

export async function getUserDashboardSnapshot() {
  return clone(userDashboardSnapshot);
}

export async function getExchangeSnapshot() {
  return clone(exchangeSnapshot);
}

export async function getStrategiesSnapshot() {
  return clone(strategiesSnapshot);
}

export async function getStrategyDetailSnapshot(id: string) {
  return clone(strategyDetailSnapshots[id as keyof typeof strategyDetailSnapshots] ?? null);
}

export async function getBillingSnapshot() {
  return clone(billingSnapshot);
}

export async function getAnalyticsSnapshot() {
  return clone(analyticsSnapshot);
}

export async function getSecuritySnapshot() {
  return clone(securitySnapshot);
}

export async function getMembershipSnapshot() {
  return clone(membershipSnapshot);
}

export async function getNotificationsSnapshot() {
  return clone(notificationsSnapshot);
}

export async function getAdminDashboardSnapshot() {
  return clone(adminDashboardSnapshot);
}

export async function getAdminUsersSnapshot() {
  return clone(adminUsersSnapshot);
}

export async function getAdminAddressPoolsSnapshot() {
  return clone(adminAddressPoolsSnapshot);
}

export async function getAdminTemplatesSnapshot() {
  return clone(adminTemplatesSnapshot);
}

export async function getAdminBillingSnapshot() {
  return clone(adminBillingSnapshot);
}

export async function getAdminAuditSnapshot() {
  return clone(adminAuditSnapshot);
}
