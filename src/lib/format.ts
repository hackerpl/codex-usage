import type { UsageWindowView } from "./types";

function secondsFromNow(unixSeconds: number): number {
  return unixSeconds - Math.floor(Date.now() / 1000);
}

function formatDuration(seconds: number): string {
  if (seconds <= 0) {
    return "now";
  }

  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (days > 0) {
    return hours > 0 ? `${days}d ${hours}h` : `${days}d`;
  }

  if (hours > 0) {
    return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`;
  }

  if (minutes > 0) {
    return `${minutes}m`;
  }

  return `${seconds}s`;
}

export function formatUpdatedAt(unixSeconds: number | null): string {
  if (!unixSeconds) {
    return "No usage snapshot yet";
  }

  const delta = Math.max(0, Math.abs(secondsFromNow(unixSeconds)));

  if (delta < 60) {
    return `Updated ${delta}s ago`;
  }

  if (delta < 3600) {
    return `Updated ${Math.floor(delta / 60)}m ago`;
  }

  if (delta < 86400) {
    return `Updated ${Math.floor(delta / 3600)}h ago`;
  }

  return `Updated ${Math.floor(delta / 86400)}d ago`;
}

export function formatResetLabel(window: UsageWindowView | null): string {
  if (!window?.resetsAt) {
    return "Waiting for the next usage snapshot";
  }

  return `Resets in ${formatDuration(secondsFromNow(window.resetsAt))}`;
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

export function formatServiceRuntime(runtime: string): string {
  switch (runtime) {
    case "running":
      return "Running";
    case "stopped":
      return "Stopped";
    case "not-installed":
      return "Not installed";
    case "unsupported":
      return "Unsupported";
    default:
      return "Unknown";
  }
}

export function formatUsageSource(source: string): string {
  switch (source) {
    case "local":
      return "Local sessions";
    case "api-configured":
      return "API configured";
    default:
      return "Stored snapshot";
  }
}

export function formatTimestampMs(value: number | null): string {
  if (!value) {
    return "None";
  }

  return new Date(value).toLocaleString();
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
