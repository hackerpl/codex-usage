import type { AppSnapshot, UsageWindowView } from "./types";

function makeWindow(
  usedPercent: number,
  resetInSeconds: number,
  windowMinutes: number,
): UsageWindowView {
  const now = Math.floor(Date.now() / 1000);

  return {
    usedPercent,
    remainingPercent: Math.max(0, Math.min(100, Math.trunc(100 - usedPercent))),
    resetsAt: now + resetInSeconds,
    windowMinutes,
  };
}

const now = Math.floor(Date.now() / 1000);

export const mockSnapshot: AppSnapshot = {
  codexHome: "~/.codex",
  registryPath: "~/.codex/accounts/registry.json",
  activeAuthPath: "~/.codex/auth.json",
  accountsDir: "~/.codex/accounts",
  registryFound: true,
  currentAccount: {
    accountKey: "active::team",
    email: "alexandra.work@gmail.com",
    alias: null,
    plan: "team",
    isActive: true,
    usage5h: makeWindow(44, 3 * 3600 + 16 * 60, 300),
    usageWeekly: makeWindow(87, 5 * 24 * 3600, 10080),
    lastUsageAt: now - 10,
  },
  otherAccounts: [
    {
      accountKey: "plus::one",
      email: "brenda.notes@gmail.com",
      alias: null,
      plan: "plus",
      isActive: false,
      usage5h: makeWindow(0, 5 * 3600, 300),
      usageWeekly: makeWindow(90, 6 * 24 * 3600, 10080),
      lastUsageAt: now - 90,
    },
    {
      accountKey: "team::two",
      email: "ryan.build@gmail.com",
      alias: null,
      plan: "team",
      isActive: false,
      usage5h: makeWindow(33, 4 * 3600 + 10 * 60, 300),
      usageWeekly: makeWindow(61, 3 * 24 * 3600, 10080),
      lastUsageAt: now - 180,
    },
    {
      accountKey: "plus::three",
      email: "carol.lab@gmail.com",
      alias: null,
      plan: "plus",
      isActive: false,
      usage5h: makeWindow(72, 2 * 3600 + 8 * 60, 300),
      usageWeekly: makeWindow(24, 4 * 24 * 3600, 10080),
      lastUsageAt: now - 220,
    },
    {
      accountKey: "pro::four",
      email: "robin.design@gmail.com",
      alias: null,
      plan: "pro",
      isActive: false,
      usage5h: makeWindow(17, 4 * 3600 + 52 * 60, 300),
      usageWeekly: makeWindow(49, 2 * 24 * 3600, 10080),
      lastUsageAt: now - 310,
    },
    {
      accountKey: "team::five",
      email: "ben.ops@gmail.com",
      alias: null,
      plan: "team",
      isActive: false,
      usage5h: makeWindow(5, 4 * 3600 + 30 * 60, 300),
      usageWeekly: makeWindow(12, 2 * 24 * 3600 + 8 * 3600, 10080),
      lastUsageAt: now - 390,
    },
  ],
  autoSwitch: {
    enabled: true,
    threshold5hPercent: 10,
    thresholdWeeklyPercent: 5,
  },
  apiUsageEnabled: true,
  usageSource: "local",
  serviceRuntime: "running",
  activeAccountActivatedAtMs: now * 1000 - 8 * 60 * 1000,
  lastLocalRolloutPath: "~/.codex/sessions/2026/03/21/rollout-demo.jsonl",
  lastLocalRolloutEventAtMs: now * 1000 - 12 * 1000,
  lastUpdatedAt: now - 10,
  warnings: ["Running outside Tauri. Showing sample data."],
  usingMock: true,
};
