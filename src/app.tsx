import { startTransition, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import {
  formatPercent,
  formatPlan,
  formatResetLabel,
  formatServiceRuntime,
  formatTimestampMs,
  formatUpdatedAt,
  formatUsageSource,
  maskEmail,
  planTone,
} from "./lib/format";
import {
  getAppSnapshot,
  launchAddAccountLogin,
  manageAutoSwitchService,
  switchAccount,
  updateSettings,
} from "./lib/tauri";
import type {
  AccountSummary,
  AppSnapshot,
  ServiceAction,
  SettingsUpdate,
  UsageWindowView,
} from "./lib/types";

type PanelMode = "add" | "status" | "settings" | null;
const STATE_INVALIDATED_EVENT = "codex://state-invalidated";

export function App() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showEmails, setShowEmails] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [switchingKey, setSwitchingKey] = useState<string | null>(null);
  const [panelMode, setPanelMode] = useState<PanelMode>(null);
  const [isSavingSettings, setIsSavingSettings] = useState(false);
  const [isLaunchingLogin, setIsLaunchingLogin] = useState(false);
  const [isRunningServiceAction, setIsRunningServiceAction] = useState(false);
  const [loginLaunchMessage, setLoginLaunchMessage] = useState<string | null>(null);
  const [serviceActionMessage, setServiceActionMessage] = useState<string | null>(null);
  const [settingsDraft, setSettingsDraft] = useState<SettingsUpdate | null>(null);
  const queuedRefreshTimer = useRef<number | null>(null);

  useEffect(() => {
    void refresh(true);
  }, []);

  useEffect(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];

    function queueRefresh(delayMs = 180) {
      if (queuedRefreshTimer.current !== null) {
        window.clearTimeout(queuedRefreshTimer.current);
      }

      queuedRefreshTimer.current = window.setTimeout(() => {
        queuedRefreshTimer.current = null;
        void refresh();
      }, delayMs);
    }

    async function attachWindowListeners() {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const currentWindow = getCurrentWindow();
        const detachFocus = await currentWindow.onFocusChanged(({ payload }) => {
          if (payload) {
            queueRefresh(0);
          }
        });
        const detachInvalidate = await currentWindow.listen<string>(STATE_INVALIDATED_EVENT, () => {
          queueRefresh();
        });

        if (disposed) {
          void detachFocus();
          void detachInvalidate();
          return;
        }

        unlisteners.push(detachFocus, detachInvalidate);
      } catch {
        // Browser preview and non-Tauri runtimes do not expose window events.
      }
    }

    void attachWindowListeners();

    return () => {
      disposed = true;
      if (queuedRefreshTimer.current !== null) {
        window.clearTimeout(queuedRefreshTimer.current);
        queuedRefreshTimer.current = null;
      }
      for (const unlisten of unlisteners) {
        void unlisten();
      }
    };
  }, []);

  async function refresh(initial = false) {
    if (initial) {
      setIsLoading(true);
    } else {
      setIsRefreshing(true);
    }

    try {
      const next = await getAppSnapshot();
      startTransition(() => {
        setSnapshot(next);
        setError(null);
      });
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setIsLoading(false);
      setIsRefreshing(false);
    }
  }

  async function handleSwitch(account: AccountSummary) {
    setSwitchingKey(account.accountKey);

    try {
      const next = await switchAccount(account.accountKey);
      startTransition(() => {
        setSnapshot(next);
        setError(null);
      });
    } catch (switchError) {
      setError(String(switchError));
    } finally {
      setSwitchingKey(null);
    }
  }

  function openSettingsPanel() {
    if (!snapshot) {
      return;
    }

    setServiceActionMessage(null);
    setSettingsDraft({
      autoSwitchEnabled: snapshot.autoSwitch.enabled,
      threshold5hPercent: snapshot.autoSwitch.threshold5hPercent,
      thresholdWeeklyPercent: snapshot.autoSwitch.thresholdWeeklyPercent,
      apiUsageEnabled: snapshot.apiUsageEnabled,
    });
    setPanelMode("settings");
  }

  function openAddAccountPanel() {
    setLoginLaunchMessage(null);
    setPanelMode("add");
  }

  async function handleSaveSettings() {
    if (!settingsDraft) {
      return;
    }

    setIsSavingSettings(true);

    try {
      const next = await updateSettings({
        ...settingsDraft,
        threshold5hPercent: clampPercent(settingsDraft.threshold5hPercent),
        thresholdWeeklyPercent: clampPercent(settingsDraft.thresholdWeeklyPercent),
      });
      startTransition(() => {
        setSnapshot(next);
        setError(null);
      });
      setPanelMode(null);
    } catch (saveError) {
      setError(String(saveError));
    } finally {
      setIsSavingSettings(false);
    }
  }

  async function handleServiceAction(action: ServiceAction) {
    setIsRunningServiceAction(true);

    try {
      const result = await manageAutoSwitchService(action);
      startTransition(() => {
        setSnapshot(result.snapshot);
        setError(null);
      });
      setServiceActionMessage(result.message);
    } catch (serviceError) {
      setError(String(serviceError));
    } finally {
      setIsRunningServiceAction(false);
    }
  }

  async function handleLaunchAddAccount() {
    setIsLaunchingLogin(true);

    try {
      const message = await launchAddAccountLogin();
      setLoginLaunchMessage(message);
      setError(null);
    } catch (launchError) {
      setError(String(launchError));
    } finally {
      setIsLaunchingLogin(false);
    }
  }

  const current = snapshot?.currentAccount ?? null;
  const accountCount = (snapshot?.otherAccounts.length ?? 0) + (current ? 1 : 0);

  return (
    <main className="app-shell">
      <section className="panel">
        <div className="panel-anchor" aria-hidden="true" data-tauri-drag-region />
        <header className="topbar">
          <div className="topbar-copy" data-tauri-drag-region>
            <h1>Codex</h1>
            <p>{snapshot ? formatUpdatedAt(snapshot.lastUpdatedAt) : "Loading local state..."}</p>
          </div>
          <button
            type="button"
            className="icon-button"
            onClick={() => void refresh()}
            disabled={isRefreshing || isLoading}
            aria-label="Refresh snapshot"
          >
            {isRefreshing ? "..." : "R"}
          </button>
        </header>
        <div className="panel-scroll">
          {error ? <div className="banner banner-error">{error}</div> : null}
          {snapshot?.warnings.map((warning) => (
            <div className="banner" key={warning}>
              {warning}
            </div>
          ))}

          {isLoading || !snapshot ? (
            <section className="empty-state">
              <p>Reading local Codex data...</p>
            </section>
          ) : (
            <>
              <section className="hero-card">
                <div className="hero-header">
                  <div>
                    <div className="hero-email">
                      {current ? maskEmail(current.email, showEmails) : "No active account"}
                    </div>
                    <div className="hero-caption">
                      {current ? current.accountKey : snapshot.registryPath}
                    </div>
                  </div>
                  {current ? (
                    <span className={`plan-pill ${planTone(current.plan)}`}>{formatPlan(current.plan)}</span>
                  ) : null}
                </div>

                <UsageSection title="5 Hours" window={current?.usage5h ?? null} />
                <UsageSection title="Weekly" window={current?.usageWeekly ?? null} />
              </section>

              <section className="account-list">
                <div className="section-title">
                  <span>Switch Account</span>
                  <span className="section-meta">{accountCount} tracked</span>
                </div>
                {snapshot.otherAccounts.length === 0 ? (
                  <div className="empty-inline">No alternate accounts in the registry yet.</div>
                ) : (
                  snapshot.otherAccounts.map((account) => (
                    <button
                      type="button"
                      key={account.accountKey}
                      className="account-row"
                      onClick={() => void handleSwitch(account)}
                      disabled={switchingKey === account.accountKey}
                    >
                      <div className="account-row-head">
                        <span>{maskEmail(account.email, showEmails)}</span>
                        <span className={`plan-mini ${planTone(account.plan)}`}>{formatPlan(account.plan)}</span>
                      </div>
                      <div className="account-metrics">
                        <MiniUsage label="5h" window={account.usage5h} />
                        <MiniUsage label="wk" window={account.usageWeekly} />
                      </div>
                    </button>
                  ))
                )}
              </section>

              <section className="actions">
                <ActionButton
                  label="Add Account"
                  detail="Start Codex sign-in"
                  onClick={openAddAccountPanel}
                />
                <ActionButton
                  label={showEmails ? "Hide Emails" : "Show Emails"}
                  detail={showEmails ? "Mask local parts" : "Reveal local parts"}
                  onClick={() => setShowEmails((value) => !value)}
                />
                <ActionButton
                  label="Status Page"
                  detail={`${formatServiceRuntime(snapshot.serviceRuntime)} · ${formatUsageSource(snapshot.usageSource)}`}
                  onClick={() => setPanelMode("status")}
                />
                <ActionButton
                  label="Settings"
                  detail={`Auto ${snapshot.autoSwitch.enabled ? "on" : "off"}`}
                  onClick={openSettingsPanel}
                />
              </section>
            </>
          )}
        </div>
      </section>

      {panelMode === "add" ? (
        <Modal title="Add Account" onClose={() => setPanelMode(null)}>
          <p className="modal-copy">
            Codex Usage can launch the sign-in flow directly. A terminal window will open for the
            native <code>codex login</code> flow, and this window will refresh itself when your
            local auth state changes.
          </p>
          {loginLaunchMessage ? <div className="banner">{loginLaunchMessage}</div> : null}
          <div className="modal-actions">
            <button
              type="button"
              className="primary-button"
              onClick={() => void handleLaunchAddAccount()}
              disabled={isLaunchingLogin}
            >
              {isLaunchingLogin ? "Launching..." : "Start Login"}
            </button>
            <button
              type="button"
              className="secondary-button"
              onClick={() => void refresh()}
            >
              Check Again
            </button>
          </div>
        </Modal>
      ) : null}

      {panelMode === "status" && snapshot ? (
        <Modal title="Status" onClose={() => setPanelMode(null)}>
          <div className="status-grid">
            <StatusRow label="Service" value={formatServiceRuntime(snapshot.serviceRuntime)} />
            <StatusRow label="Usage Mode" value={snapshot.apiUsageEnabled ? "API configured" : "Local mode"} />
            <StatusRow label="Displayed Source" value={formatUsageSource(snapshot.usageSource)} />
            <StatusRow label="Auto Switch" value={snapshot.autoSwitch.enabled ? "Enabled" : "Disabled"} />
            <StatusRow label="5h Threshold" value={`${snapshot.autoSwitch.threshold5hPercent}%`} />
            <StatusRow label="Weekly Threshold" value={`${snapshot.autoSwitch.thresholdWeeklyPercent}%`} />
            <StatusRow label="Accounts" value={`${accountCount}`} />
            {snapshot.currentAccount ? (
              <StatusRow label="Current Account" value={maskEmail(snapshot.currentAccount.email, showEmails)} />
            ) : null}
            <StatusRow label="Active Since" value={formatTimestampMs(snapshot.activeAccountActivatedAtMs)} />
            <StatusRow label="Local Rollout At" value={formatTimestampMs(snapshot.lastLocalRolloutEventAtMs)} />
            <StatusRow
              label="Local Rollout File"
              value={snapshot.lastLocalRolloutPath ?? "None"}
              multiline
            />
            <StatusRow label="Codex Home" value={snapshot.codexHome} multiline />
            <StatusRow label="Registry" value={snapshot.registryPath} multiline />
            <StatusRow label="Active Auth" value={snapshot.activeAuthPath} multiline />
            <StatusRow label="Accounts Dir" value={snapshot.accountsDir} multiline />
          </div>
          <div className="modal-footer-note">
            External status page:{" "}
            <a href="https://status.openai.com" target="_blank" rel="noreferrer">
              https://status.openai.com
            </a>
          </div>
        </Modal>
      ) : null}

      {panelMode === "settings" && snapshot && settingsDraft ? (
        <Modal title="Settings" onClose={() => setPanelMode(null)}>
          <div className="settings-grid">
            <label className="toggle-row">
              <span>Auto switch</span>
              <input
                type="checkbox"
                checked={settingsDraft.autoSwitchEnabled}
                onChange={(event) =>
                  setSettingsDraft((draft) =>
                    draft
                      ? {
                          ...draft,
                          autoSwitchEnabled: event.target.checked,
                        }
                      : draft,
                  )
                }
              />
            </label>

            <label className="toggle-row">
              <span>Record API usage mode</span>
              <input
                type="checkbox"
                checked={settingsDraft.apiUsageEnabled}
                onChange={(event) =>
                  setSettingsDraft((draft) =>
                    draft
                      ? {
                          ...draft,
                          apiUsageEnabled: event.target.checked,
                        }
                      : draft,
                  )
                }
              />
            </label>

            <label className="field-row">
              <span>5h threshold</span>
              <input
                type="number"
                min={1}
                max={100}
                value={settingsDraft.threshold5hPercent}
                onChange={(event) =>
                  setSettingsDraft((draft) =>
                    draft
                      ? {
                          ...draft,
                          threshold5hPercent: Number(event.target.value || 1),
                        }
                      : draft,
                  )
                }
              />
            </label>

            <label className="field-row">
              <span>Weekly threshold</span>
              <input
                type="number"
                min={1}
                max={100}
                value={settingsDraft.thresholdWeeklyPercent}
                onChange={(event) =>
                  setSettingsDraft((draft) =>
                    draft
                      ? {
                          ...draft,
                          thresholdWeeklyPercent: Number(event.target.value || 1),
                        }
                      : draft,
                  )
                }
              />
            </label>
          </div>

          <section className="service-card">
            <div className="service-card-head">
              <div>
                <h4>Background Service</h4>
                <p>Owns the Linux user timer for automatic checks.</p>
              </div>
              <span className="service-runtime-badge">{formatServiceRuntime(snapshot.serviceRuntime)}</span>
            </div>
            {serviceActionMessage ? <div className="banner">{serviceActionMessage}</div> : null}
            <div className="service-actions">
              {serviceActionsForRuntime(snapshot.serviceRuntime).map((item) => (
                <button
                  key={item.action}
                  type="button"
                  className={item.primary ? "primary-button" : "secondary-button"}
                  onClick={() => void handleServiceAction(item.action)}
                  disabled={isRunningServiceAction}
                >
                  {isRunningServiceAction ? "Working..." : item.label}
                </button>
              ))}
            </div>
          </section>

          <div className="modal-actions">
            <button type="button" className="secondary-button" onClick={() => setPanelMode(null)}>
              Cancel
            </button>
            <button
              type="button"
              className="primary-button"
              onClick={() => void handleSaveSettings()}
              disabled={isSavingSettings}
            >
              {isSavingSettings ? "Saving..." : "Save Settings"}
            </button>
          </div>
        </Modal>
      ) : null}
    </main>
  );
}

function UsageSection({
  title,
  window,
}: {
  title: string;
  window: UsageWindowView | null;
}) {
  const value = window?.remainingPercent ?? 0;

  return (
    <section className="usage-block">
      <div className="usage-head">
        <h2>{title}</h2>
        <span>{formatPercent(window?.remainingPercent)}</span>
      </div>
      <div className="meter">
        <span style={{ width: `${value}%` }} />
      </div>
      <p>{formatResetLabel(window)}</p>
    </section>
  );
}

function MiniUsage({
  label,
  window,
}: {
  label: string;
  window: UsageWindowView | null;
}) {
  const value = window?.remainingPercent ?? 0;

  return (
    <div className="mini-usage">
      <span className="mini-label">{label}</span>
      <div className="mini-meter">
        <span style={{ width: `${value}%` }} />
      </div>
      <span className="mini-value">{formatPercent(window?.remainingPercent)}</span>
    </div>
  );
}

function ActionButton({
  label,
  detail,
  onClick,
  disabled,
}: {
  label: string;
  detail: string;
  onClick?: () => void;
  disabled?: boolean;
}) {
  return (
    <button type="button" className="action-row" onClick={onClick} disabled={disabled}>
      <span>{label}</span>
      <span>{detail}</span>
    </button>
  );
}

function Modal({
  title,
  children,
  onClose,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
}) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <section className="modal-card" onClick={(event) => event.stopPropagation()}>
        <header className="modal-header">
          <h3>{title}</h3>
          <button type="button" className="icon-button" onClick={onClose} aria-label="Close panel">
            X
          </button>
        </header>
        {children}
      </section>
    </div>
  );
}

function StatusRow({
  label,
  value,
  multiline,
}: {
  label: string;
  value: string;
  multiline?: boolean;
}) {
  return (
    <div className={`status-row ${multiline ? "status-row-multiline" : ""}`}>
      <span>{label}</span>
      <span>{value}</span>
    </div>
  );
}

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) {
    return 1;
  }

  return Math.max(1, Math.min(100, Math.round(value)));
}

function serviceActionsForRuntime(runtime: string): Array<{
  action: ServiceAction;
  label: string;
  primary?: boolean;
}> {
  switch (runtime) {
    case "not-installed":
      return [
        { action: "install", label: "Install & Start", primary: true },
        { action: "run-now", label: "Run Check Now" },
      ];
    case "running":
      return [
        { action: "run-now", label: "Run Check Now", primary: true },
        { action: "stop", label: "Stop Service" },
        { action: "uninstall", label: "Uninstall" },
      ];
    case "stopped":
      return [
        { action: "start", label: "Start Service", primary: true },
        { action: "run-now", label: "Run Check Now" },
        { action: "uninstall", label: "Uninstall" },
      ];
    default:
      return [{ action: "run-now", label: "Run Check Now", primary: true }];
  }
}
