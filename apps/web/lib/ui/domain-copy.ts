import { pickText, type UiLanguage } from "./preferences";

export function describeMembershipStatus(lang: UiLanguage, status?: string | null) {
  switch (status) {
    case "Active":
      return pickText(lang, "已生效", "Active");
    case "Grace":
      return pickText(lang, "宽限期", "Grace");
    case "Expired":
      return pickText(lang, "已过期", "Expired");
    case "Frozen":
      return pickText(lang, "已冻结", "Frozen");
    case "Revoked":
      return pickText(lang, "已撤销", "Revoked");
    case "Pending":
      return pickText(lang, "待开通", "Pending");
    default:
      return status ?? pickText(lang, "未知", "Unknown");
  }
}

export function localizeNotificationTitle(
  lang: UiLanguage,
  kind?: string | null,
  fallback?: string | null,
) {
  switch (kind) {
    case "MembershipExpiring":
      return pickText(lang, "会员到期提醒", "Membership expiry alert");
    case "DepositConfirmed":
      return pickText(lang, "充值已确认", "Deposit confirmed");
    case "StrategyStarted":
      return pickText(lang, "策略已启动", "Strategy started");
    case "StrategyPaused":
      return pickText(lang, "策略已暂停", "Strategy paused");
    case "RuntimeErrorAutoPaused":
      return pickText(lang, "策略异常自动暂停", "Strategy auto-paused on runtime error");
    case "ApiCredentialsInvalidated":
      return pickText(lang, "交易所凭证失效", "API credentials invalid");
    case "OverallTakeProfitTriggered":
      return pickText(lang, "整体止盈触发", "Overall take profit triggered");
    case "OverallStopLossTriggered":
      return pickText(lang, "整体止损触发", "Overall stop loss triggered");
    case "GridFillExecuted":
      return pickText(lang, "网格成交", "Grid fill executed");
    case "FillProfitReported":
      return pickText(lang, "单笔收益更新", "Fill profit update");
    default:
      return fallback ?? pickText(lang, "系统通知", "Notification");
  }
}

export function localizeNotificationMessage(
  lang: UiLanguage,
  kind?: string | null,
  fallback?: string | null,
) {
  switch (kind) {
    case "MembershipExpiring":
      return pickText(lang, "会员宽限期已结束，系统已暂停相关策略。", "The membership grace window ended and related strategies were paused.");
    case "DepositConfirmed":
      return pickText(lang, "会员充值已确认到账，会员时长已更新。", "The membership deposit is confirmed and the entitlement has been extended.");
    case "ApiCredentialsInvalidated":
      return pickText(lang, "币安凭证校验失败，请到交易所页面重新检查 API 权限与连接状态。", "Binance credential validation failed. Review API permissions and account connectivity.");
    default:
      return fallback ?? pickText(lang, "请在页面中查看详情。", "Open the related page for details.");
  }
}
