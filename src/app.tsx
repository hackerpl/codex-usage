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
import type { Lang } from "./lib/format";
import {
  getAppSnapshot,
  launchAddAccountLogin,
  manageAutoSwitchService,
  removeAccount,
  switchAccount,
  updateSettings,
  quitApp,
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
  const [removingKey, setRemovingKey] = useState<string | null>(null);
  const [panelMode, setPanelMode] = useState<PanelMode>(null);
  const [pendingRemoval, setPendingRemoval] = useState<AccountSummary | null>(null);
  const [isSavingSettings, setIsSavingSettings] = useState(false);
  const [isLaunchingLogin, setIsLaunchingLogin] = useState(false);
  const [isRunningServiceAction, setIsRunningServiceAction] = useState(false);
  const [loginLaunchMessage, setLoginLaunchMessage] = useState<string | null>(null);
  const [serviceActionMessage, setServiceActionMessage] = useState<string | null>(null);
  const [settingsDraft, setSettingsDraft] = useState<SettingsUpdate | null>(null);
  const queuedRefreshTimer = useRef<number | null>(null);

  const [lang, setLang] = useState<Lang>("zh");
  const t = (en: string, zh: string) => lang === "zh" ? zh : en;

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

  async function handleRemoveAccount(account: AccountSummary) {
    setRemovingKey(account.accountKey);

    try {
      const next = await removeAccount(account.accountKey);
      startTransition(() => {
        setSnapshot(next);
        setError(null);
      });
      setPendingRemoval(null);
    } catch (removeError) {
      setError(String(removeError));
    } finally {
      setRemovingKey(null);
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
  const pendingRemovalIsCurrent = pendingRemoval?.isActive ?? false;
  const pendingRemovalWillClearActiveAuth = accountCount <= 1;

  function isAccountBusy(accountKey: string): boolean {
    return switchingKey === accountKey || removingKey === accountKey;
  }

  return (
    <main className="app-shell">
      <section className="panel">
        <div className="panel-anchor" aria-hidden="true" data-tauri-drag-region />
        <header className="topbar">
          <div className="topbar-copy" data-tauri-drag-region>
            <h1>Codex</h1>
            <p>{snapshot ? formatUpdatedAt(snapshot.lastUpdatedAt, lang) : t("Loading local state...", "正在载入本地状态...")}</p>
          </div>
          <div className="topbar-actions">
            <button
              type="button"
              className="icon-button icon-button-wide"
              onClick={() => setLang(lang === "zh" ? "en" : "zh")}
              aria-label={t("Switch language", "切换语言")}
            >
              {lang === "zh" ? "EN" : "中"}
            </button>
            <button
              type="button"
              className="icon-button"
              onClick={() => void refresh()}
              disabled={isRefreshing || isLoading}
              aria-label={t("Refresh snapshot", "刷新快照")}
            >
              {isRefreshing ? "..." : "R"}
            </button>
          </div>
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
              <p>{t("Reading local Codex data...", "正在读取本地用量信息...")}</p>
            </section>
          ) : (
            <>
              <header className="v2-topbar">
                <div className="v2-top-row">
                  <h1 className="v2-title">Codex</h1>
                  <div className="v2-top-right">
                    {current ? <span className="v2-current-email">{maskEmail(current.email, showEmails)}</span> : <span className="v2-current-email">{t("No active account", "无激活用量")}</span>}
                    <button type="button" className="v2-refresh-btn" onClick={() => void refresh()} disabled={isRefreshing || isLoading} aria-label="Refresh">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3" /></svg>
                    </button>
                  </div>
                </div>
                <div className="v2-sub-row">
                  <span className="v2-updated">{formatUpdatedAt(snapshot.lastUpdatedAt, lang)}</span>
                  <div className="v2-sub-actions">
                    {current ? <span className="v2-plan">{formatPlan(current.plan)}</span> : <span className="v2-plan" />}
                    {current ? (
                      <button
                        type="button"
                        className="row-action-button row-action-button-danger"
                        onClick={() => setPendingRemoval(current)}
                        disabled={isAccountBusy(current.accountKey)}
                      >
                        {removingKey === current.accountKey ? t("Removing...", "移除中...") : t("Remove", "移除")}
                      </button>
                    ) : null}
                  </div>
                </div>
              </header>

              <div className="v2-divider" />

              <UsageSection title={t("5 Hours", "近5小时用量")} window={current?.usage5h ?? null} lang={lang} />

              <div className="v2-divider-light" />

              <UsageSection title={t("Weekly", "近7天用量")} window={current?.usageWeekly ?? null} lang={lang} />

              <div className="v2-divider" />

              <section className="v2-account-list">
                <h2 className="v2-section-title">{t("Switch Account", "切换账号")}</h2>
                {snapshot.otherAccounts.length === 0 ? (
                  <div className="empty-inline">{t("No other accounts in the registry yet.", "登记簿中暂无其他账号。")}</div>
                ) : (
                  snapshot.otherAccounts.map((account) => (
                    <div
                      key={account.accountKey}
                      className="account-row"
                    >
                      <div className="account-row-head">
                        <span className="v2-acc-email">{maskEmail(account.email, showEmails)}</span>
                        <span className={`plan-mini ${planTone(account.plan)}`}>{formatPlan(account.plan)}</span>
                      </div>
                      <div className="v2-acc-bottom">
                        <span className="v2-mini-lbl">5h</span>
                        <div className="v2-mini-meter"><div className="v2-fill" style={{ width: `${account.usage5h?.remainingPercent ?? 0}%` }} /></div>
                        <span className="v2-mini-val">{formatPercent(account.usage5h?.remainingPercent)}</span>

                        <span className="v2-mini-lbl v2-spacer">wk</span>
                        <div className="v2-mini-meter"><div className="v2-fill" style={{ width: `${account.usageWeekly?.remainingPercent ?? 0}%` }} /></div>
                        <span className="v2-mini-val">{formatPercent(account.usageWeekly?.remainingPercent)}</span>
                      </div>
                      <div className="account-row-actions">
                        <button
                          type="button"
                          className="row-action-button"
                          onClick={() => void handleSwitch(account)}
                          disabled={isAccountBusy(account.accountKey)}
                        >
                          {switchingKey === account.accountKey ? t("Switching...", "切换中...") : t("Switch", "切换")}
                        </button>
                        <button
                          type="button"
                          className="row-action-button row-action-button-danger"
                          onClick={() => setPendingRemoval(account)}
                          disabled={isAccountBusy(account.accountKey)}
                        >
                          {removingKey === account.accountKey ? t("Removing...", "移除中...") : t("Remove", "移除")}
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </section>

              <div className="v2-divider" />

              <section className="v2-actions">
                <ActionButton
                  icon={<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>}
                  label={t("Add Account", "添加账号")}
                  onClick={openAddAccountPanel}
                />
                <ActionButton
                  icon={<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 2A10 10 0 0 0 2 12a10 10 0 0 0 10 10 10 10 0 0 0 10-10A10 10 0 0 0 12 2Z"></path><path d="M12 16v-4"></path><path d="M12 8h.01"></path></svg>}
                  label={t("Status Page", "查看状态详情")}
                  onClick={() => setPanelMode("status")}
                />
                <ActionButton
                  icon={showEmails ? <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"></path><line x1="1" y1="1" x2="23" y2="23"></line></svg> : <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path><circle cx="12" cy="12" r="3"></circle></svg>}
                  label={showEmails ? t("Hide Emails", "隐藏完整邮箱") : t("Show Emails", "显示完整邮箱")}
                  onClick={() => setShowEmails((value) => !value)}
                />
                <ActionButton
                  icon={<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>}
                  label={t("Settings", "偏好设置")}
                  onClick={openSettingsPanel}
                />
                <ActionButton
                  icon={<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M18.36 6.64a9 9 0 1 1-12.73 0"></path><line x1="12" y1="2" x2="12" y2="12"></line></svg>}
                  label={t("Quit", "退出应用")}
                  onClick={() => { void quitApp() }}
                />
              </section>
            </>
          )}
        </div>
      </section>

      {panelMode === "add" ? (
        <Modal title={t("Add Account", "添加账号")} onClose={() => setPanelMode(null)}>
          <p className="modal-copy">
            {t(
              "Codex Usage can launch the sign-in flow directly. A terminal window will open for the native codex login flow...",
              "您可以通过该辅助面板直接拉起原生 CLI 终端执行一次授权登录（基于本机凭证系统）。此窗口将在系统级认证结束后被自动刷新接管记录。"
            )}
          </p>
          {loginLaunchMessage ? <div className="banner">{loginLaunchMessage}</div> : null}
          <div className="modal-actions">
            <button
              type="button"
              className="primary-button"
              onClick={() => void handleLaunchAddAccount()}
              disabled={isLaunchingLogin}
            >
              {isLaunchingLogin ? t("Launching...", "正在拉起终端...") : t("Start Login", "开始拉起命令行登录")}
            </button>
            <button
              type="button"
              className="secondary-button"
              onClick={() => void refresh()}
            >
              {t("Check Again", "刷新凭证识别")}
            </button>
          </div>
        </Modal>
      ) : null}

      {panelMode === "status" && snapshot ? (
        <Modal title={t("Status", "核心状态")} onClose={() => setPanelMode(null)}>
          <div className="status-grid">
            <StatusRow label={t("Service", "系统守护进程")} value={formatServiceRuntime(snapshot.serviceRuntime, lang)} />
            <StatusRow label={t("Usage Mode", "当前获取模式")} value={snapshot.apiUsageEnabled ? t("API configured", "API 被动请求") : t("Local mode", "本地流日志监听")} />
            <StatusRow label={t("Displayed Source", "数据流来源")} value={formatUsageSource(snapshot.usageSource, lang)} />
            <StatusRow label={t("Auto Switch", "后备自动漂移")} value={snapshot.autoSwitch.enabled ? t("Enabled", "已开启") : t("Disabled", "已关闭禁用")} />
            <StatusRow label={t("5h Threshold", "单5小时健康阈值")} value={`${snapshot.autoSwitch.threshold5hPercent}%`} />
            <StatusRow label={t("Weekly Threshold", "单7天健康阈值")} value={`${snapshot.autoSwitch.thresholdWeeklyPercent}%`} />
            <StatusRow label={t("Accounts", "备源收录")} value={`${accountCount}`} />
            {snapshot.currentAccount ? (
              <StatusRow label={t("Current Account", "正在消耗账号")} value={maskEmail(snapshot.currentAccount.email, showEmails)} />
            ) : null}
            <StatusRow label={t("Active Since", "上游接管点")} value={formatTimestampMs(snapshot.activeAccountActivatedAtMs, lang)} />
            <StatusRow label={t("Local Rollout At", "本地记录锚点")} value={formatTimestampMs(snapshot.lastLocalRolloutEventAtMs, lang)} />
            <StatusRow
              label={t("Local Rollout File", "最后记录流文卷")}
              value={snapshot.lastLocalRolloutPath ?? t("None", "无记录")}
              multiline
            />
            <StatusRow label={t("Codex Home", "Codex 家目录")} value={snapshot.codexHome} multiline />
            <StatusRow label={t("Registry", "中央登记簿")} value={snapshot.registryPath} multiline />
            <StatusRow label={t("Active Auth", "活跃临时证")} value={snapshot.activeAuthPath} multiline />
            <StatusRow label={t("Accounts Dir", "证件快照仓")} value={snapshot.accountsDir} multiline />
          </div>
          <div className="modal-footer-note">
            {t("External status page: ", "外部服务器官方探针节点：")}
            <a href="https://status.openai.com" target="_blank" rel="noreferrer">
              https://status.openai.com
            </a>
          </div>
        </Modal>
      ) : null}

      {pendingRemoval ? (
        <Modal
          title={t("Remove Account", "移除账号")}
          onClose={() => {
            if (!removingKey) {
              setPendingRemoval(null);
            }
          }}
        >
          <p className="modal-copy">
            {pendingRemovalWillClearActiveAuth
              ? t(
                "This will remove the last tracked account and clear the active local auth snapshot.",
                "这会移除最后一个已登记账号，并清空当前本地激活认证快照。"
              )
              : pendingRemovalIsCurrent
                ? t(
                  "This will remove the current account and immediately switch the app to the healthiest remaining account.",
                  "这会移除当前账号，并立即切换到剩余账号里状态最好的一个。"
                )
                : t(
                  "This will delete the selected account from the local registry and remove its saved auth snapshot.",
                  "这会把所选账号从本地登记簿删除，并移除它保存的认证快照。"
                )}
          </p>
          <div className="status-grid modal-confirm-grid">
            <StatusRow label={t("Account", "账号")} value={maskEmail(pendingRemoval.email, showEmails)} />
            <StatusRow label={t("Plan", "套餐")} value={formatPlan(pendingRemoval.plan)} />
            <StatusRow label={t("Account Key", "账号键")} value={pendingRemoval.accountKey} multiline />
          </div>
          <div className="modal-actions">
            <button
              type="button"
              className="secondary-button"
              onClick={() => setPendingRemoval(null)}
              disabled={Boolean(removingKey)}
            >
              {t("Cancel", "取消")}
            </button>
            <button
              type="button"
              className="danger-button"
              onClick={() => void handleRemoveAccount(pendingRemoval)}
              disabled={Boolean(removingKey)}
            >
              {removingKey === pendingRemoval.accountKey ? t("Removing...", "移除中...") : t("Remove Account", "确认移除")}
            </button>
          </div>
        </Modal>
      ) : null}

      {panelMode === "settings" && snapshot && settingsDraft ? (
        <Modal title={t("Settings", "核心设置")} onClose={() => setPanelMode(null)}>
          <div className="settings-grid">
            <label className="toggle-row">
              <span>{t("Auto switch", "启用自动化越级切换")}</span>
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
              <span>{t("Record API usage mode", "从外部 API 被动刷新")}</span>
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
              <span>{t("5h threshold", "5H 触发阈值警戒线")}</span>
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
              <span>{t("Weekly threshold", "周均触发阈值警戒线")}</span>
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
                <h4>{t("Background Service", "常驻系统任务程序")}</h4>
                <p>{t("Owns the OS user timer for automatic checks.", "这负责在本地系统级别写入任务，来达成零活守护机制。")}</p>
              </div>
              <span className="service-runtime-badge">{formatServiceRuntime(snapshot.serviceRuntime, lang)}</span>
            </div>
            {serviceActionMessage ? <div className="banner">{serviceActionMessage}</div> : null}
            <div className="service-actions">
              {serviceActionsForRuntime(snapshot.serviceRuntime, lang).map((item) => (
                <button
                  key={item.action}
                  type="button"
                  className={item.primary ? "primary-button" : "secondary-button"}
                  onClick={() => void handleServiceAction(item.action)}
                  disabled={isRunningServiceAction}
                >
                  {isRunningServiceAction ? t("Working...", "执行中...") : item.label}
                </button>
              ))}
            </div>
          </section>

          <div className="modal-actions">
            <button type="button" className="secondary-button" onClick={() => setPanelMode(null)}>
              {t("Cancel", "直接取消")}
            </button>
            <button
              type="button"
              className="primary-button"
              onClick={() => void handleSaveSettings()}
              disabled={isSavingSettings}
            >
              {isSavingSettings ? t("Saving...", "同步登记中...") : t("Save Settings", "硬保存写入")}
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
  lang,
}: {
  title: string;
  window: UsageWindowView | null;
  lang: Lang;
}) {
  const value = window?.remainingPercent ?? 0;

  return (
    <section className="v2-usage-block">
      <div className="v2-usage-head">
        <h2>{title}</h2>
        <span>{formatPercent(window?.remainingPercent)}</span>
      </div>
      <div className="v2-meter">
        <span className="v2-fill" style={{ width: `${value}%` }} />
      </div>
      <p className="v2-usage-sub">{formatResetLabel(window, lang)}</p>
    </section>
  );
}

function ActionButton({
  icon,
  label,
  onClick,
  disabled,
}: {
  icon?: ReactNode;
  label: string;
  onClick?: () => void;
  disabled?: boolean;
}) {
  return (
    <button type="button" className="v2-action-row" onClick={onClick} disabled={disabled}>
      {icon && <span className="v2-action-icon">{icon}</span>}
      <span className="v2-action-label">{label}</span>
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
        <div className="modal-body">
          {children}
        </div>
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

function serviceActionsForRuntime(runtime: string, lang: Lang): Array<{
  action: ServiceAction;
  label: string;
  primary?: boolean;
}> {
  switch (runtime) {
    case "not-installed":
      return [
        { action: "install", label: lang === "zh" ? "安装并启动保护" : "Install & Start", primary: true },
        { action: "run-now", label: lang === "zh" ? "手动走一次轮询" : "Run Check Now" },
      ];
    case "running":
      return [
        { action: "run-now", label: lang === "zh" ? "手动走一次轮询" : "Run Check Now", primary: true },
        { action: "stop", label: lang === "zh" ? "停止底层保护" : "Stop Service" },
        { action: "uninstall", label: lang === "zh" ? "完全卸载卸下模块" : "Uninstall" },
      ];
    case "stopped":
      return [
        { action: "start", label: lang === "zh" ? "重新激活守护" : "Start Service", primary: true },
        { action: "run-now", label: lang === "zh" ? "手动走一次轮询" : "Run Check Now" },
        { action: "uninstall", label: lang === "zh" ? "完全卸载卸下模块" : "Uninstall" },
      ];
    default:
      return [{ action: "run-now", label: lang === "zh" ? "手动走一次轮询" : "Run Check Now", primary: true }];
  }
}

