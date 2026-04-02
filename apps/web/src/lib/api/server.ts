import "server-only";

import { buildAdminShellSnapshot } from "./admin-product-state";
import {
  adminAddressPoolsSnapshot,
  adminAuditSnapshot,
  adminDashboardSnapshot,
  adminDepositsSnapshot,
  adminMembershipsSnapshot,
  adminStrategiesSnapshot,
  adminSweepsSnapshot,
  adminSystemSnapshot,
  adminTemplatesSnapshot,
  adminUsersSnapshot,
  analyticsSnapshot,
  billingSnapshot,
  buildUserShellSnapshot,
  exchangeSnapshot,
  helpCenterSnapshot,
  homeSnapshot,
  membershipSnapshot,
  notificationsSnapshot,
  ordersSnapshot,
  publicAuthSnapshots,
  publicShellSnapshot,
  securitySnapshot,
  strategiesSnapshot,
  strategyComposerSnapshot,
  strategyDetailSnapshots,
  telegramSnapshot,
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

export async function getHomeSnapshot() {
  return clone(homeSnapshot);
}

export async function getPublicAuthSnapshot(mode: "login" | "register") {
  return clone(publicAuthSnapshots[mode]);
}

export async function getUserShellSnapshot(): Promise<UserShellSnapshot> {
  return clone(buildUserShellSnapshot());
}

export async function getAdminShellSnapshot(): Promise<AdminShellSnapshot> {
  return buildAdminShellSnapshot();
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

export async function getStrategyComposerSnapshot() {
  return clone(strategyComposerSnapshot);
}

export async function getStrategyDetailSnapshot(id: string) {
  return clone(strategyDetailSnapshots[id as keyof typeof strategyDetailSnapshots] ?? null);
}

export async function getOrdersSnapshot() {
  return clone(ordersSnapshot);
}

export async function getBillingSnapshot() {
  return clone(billingSnapshot);
}

export async function getAnalyticsSnapshot() {
  return clone(analyticsSnapshot);
}

export async function getTelegramSnapshot() {
  return clone(telegramSnapshot);
}

export async function getSecuritySnapshot() {
  return clone(securitySnapshot);
}

export async function getHelpCenterSnapshot() {
  return clone(helpCenterSnapshot);
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

export async function getAdminMembershipsSnapshot() {
  return clone(adminMembershipsSnapshot);
}

export async function getAdminDepositsSnapshot() {
  return clone(adminDepositsSnapshot);
}

export async function getAdminAddressPoolsSnapshot() {
  return clone(adminAddressPoolsSnapshot);
}

export async function getAdminTemplatesSnapshot() {
  return clone(adminTemplatesSnapshot);
}

export async function getAdminStrategiesSnapshot() {
  return clone(adminStrategiesSnapshot);
}

export async function getAdminSweepsSnapshot() {
  return clone(adminSweepsSnapshot);
}

export async function getAdminAuditSnapshot() {
  return clone(adminAuditSnapshot);
}

export async function getAdminSystemSnapshot() {
  return clone(adminSystemSnapshot);
}
