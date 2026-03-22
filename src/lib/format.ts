import type { UsageWindowView } from "./types";

export type Lang = "en" | "zh";

function secondsFromNow(unixSeconds: number): number {
  return unixSeconds - Math.floor(Date.now() / 1000);
}

function formatDuration(seconds: number, lang: Lang): string {
  if (seconds <= 0) {
    return lang === "zh" ? "刚刚" : "now";
  }

  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  const d = lang === "zh" ? "天" : "d";
  const h = lang === "zh" ? "小时" : "h";
  const m = lang === "zh" ? "分" : "m";
  const s = lang === "zh" ? "秒" : "s";

  if (days > 0) {
    return hours > 0 ? `${days}${d} ${hours}${h}` : `${days}${d}`;
  }

  if (hours > 0) {
    return minutes > 0 ? `${hours}${h} ${minutes}${m}` : `${hours}${h}`;
  }

  if (minutes > 0) {
    return `${minutes}${m}`;
  }

  return `${seconds}${s}`;
}

export function formatUpdatedAt(unixSeconds: number | null, lang: Lang): string {
  if (!unixSeconds) {
    return lang === "zh" ? "暂无用量快照" : "No usage snapshot yet";
  }

  const delta = Math.max(0, Math.abs(secondsFromNow(unixSeconds)));
  const updated = lang === "zh" ? "更新于" : "Updated";
  const ago = lang === "zh" ? "前" : " ago";

  if (delta < 60) {
    return `${updated} ${delta} ${lang === "zh" ? "秒" : "s"}${ago}`;
  }

  if (delta < 3600) {
    return `${updated} ${Math.floor(delta / 60)} ${lang === "zh" ? "分钟" : "m"}${ago}`;
  }

  if (delta < 86400) {
    return `${updated} ${Math.floor(delta / 3600)} ${lang === "zh" ? "小时" : "h"}${ago}`;
  }

  return `${updated} ${Math.floor(delta / 86400)} ${lang === "zh" ? "天" : "d"}${ago}`;
}

export function formatResetLabel(window: UsageWindowView | null, lang: Lang): string {
  if (!window?.resetsAt) {
    return lang === "zh" ? "等待下一次用量快照" : "Waiting for the next usage snapshot";
  }

  const dur = formatDuration(secondsFromNow(window.resetsAt), lang);
  return lang === "zh" ? `在此之后重置：${dur}` : `Resets in ${dur}`;
}

export function maskEmail(email: string, show: boolean): string {
  if (show) {
    return email;
  }

  const [local, domain] = email.split("@");
  if (!local || !domain) {
    return email;
  }

  return `${local[0]}....@${domain}`;
}

export function formatPercent(value: number | null | undefined): string {
  if (value == null) {
    return "--";
  }

  return `${Math.max(0, Math.min(100, Math.round(value)))}%`;
}

export function formatPlan(plan: string | null): string {
  if (!plan) {
    return "Unknown";
  }

  return plan.charAt(0).toUpperCase() + plan.slice(1);
}

export function formatServiceRuntime(runtime: string, lang: Lang): string {
  switch (runtime) {
    case "running":
      return lang === "zh" ? "运行中" : "Running";
    case "stopped":
      return lang === "zh" ? "已停止" : "Stopped";
    case "not-installed":
      return lang === "zh" ? "未安装" : "Not installed";
    case "unsupported":
      return lang === "zh" ? "不支持" : "Unsupported";
    default:
      return lang === "zh" ? "未知" : "Unknown";
  }
}

export function formatUsageSource(source: string, lang: Lang): string {
  switch (source) {
    case "local":
      return lang === "zh" ? "本地会话读取" : "Local sessions";
    case "api-configured":
      return lang === "zh" ? "API 已配置" : "API configured";
    default:
      return lang === "zh" ? "存储快照缓存" : "Stored snapshot";
  }
}

export function formatTimestampMs(value: number | null, lang: Lang): string {
  if (!value) {
    return lang === "zh" ? "未记录" : "None";
  }

  return new Date(value).toLocaleString(lang === "zh" ? "zh-CN" : "en-US");
}

export function planTone(plan: string | null): string {
  switch (plan) {
    case "team":
    case "business":
    case "enterprise":
      return "tone-team";
    case "pro":
      return "tone-pro";
    case "plus":
      return "tone-plus";
    case "free":
      return "tone-free";
    default:
      return "tone-unknown";
  }
}
