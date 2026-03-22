export interface UsageWindowView {
  usedPercent: number;
  remainingPercent: number;
  resetsAt: number | null;
  windowMinutes: number | null;
}

export interface AccountSummary {
  accountKey: string;
  email: string;
  alias: string | null;
  plan: string | null;
  isActive: boolean;
  usage5h: UsageWindowView | null;
  usageWeekly: UsageWindowView | null;
  lastUsageAt: number | null;
}

export interface AutoSwitchSummary {
  enabled: boolean;
  threshold5hPercent: number;
  thresholdWeeklyPercent: number;
}

export interface SettingsUpdate {
  autoSwitchEnabled: boolean;
  threshold5hPercent: number;
  thresholdWeeklyPercent: number;
  apiUsageEnabled: boolean;
}

export interface AppSnapshot {
  codexHome: string;
  registryPath: string;
  activeAuthPath: string;
  accountsDir: string;
  registryFound: boolean;
  currentAccount: AccountSummary | null;
  otherAccounts: AccountSummary[];
  autoSwitch: AutoSwitchSummary;
  apiUsageEnabled: boolean;
  usageSource: "local" | "stored" | "api-configured" | string;
  serviceRuntime: string;
  activeAccountActivatedAtMs: number | null;
  lastLocalRolloutPath: string | null;
  lastLocalRolloutEventAtMs: number | null;
  lastUpdatedAt: number | null;
  warnings: string[];
  usingMock: boolean;
}

export type ServiceAction = "install" | "start" | "stop" | "uninstall" | "run-now";

export interface ServiceActionResult {
  snapshot: AppSnapshot;
  message: string;
}
