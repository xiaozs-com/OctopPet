import { useEffect, useRef, useState } from "react";

import { APP_NAME } from "../lib/brand";
import ShortcutRecorder from "../components/ShortcutRecorder";
import WindowCloseButton from "../components/WindowCloseButton";
import {
  useAutoFitWindow,
  useEscapeHidesWindow,
} from "../hooks/useWindowChrome";
import {
  DEFAULT_APP_CONFIG,
  MASCOT_SRC,
  normalizeBaseUrl,
} from "../lib/configLogic";
import { version as appVersion } from "../../package.json";
import { login } from "../lib/octopHttp";
import { tauriApi } from "../lib/tauriApi";
import { hideCurrentWindow } from "../lib/tauriWindowApi";
import type { AppConfig, MascotId } from "../lib/types";

type Notice = { kind: "success" | "error"; text: string } | null;
type SettingsTab = "general" | "window" | "hotkeys" | "about";

const SETTINGS_WIDTH = 480;

const MASCOT_OPTIONS: Array<{ id: MascotId; label: string }> = [
  { id: "peek", label: "Peek" },
  { id: "type", label: "Type" },
  { id: "taiji-bot", label: "太极机器人" },
];

const TABS: Array<{ id: SettingsTab; label: string }> = [
  { id: "general", label: "常规" },
  { id: "window", label: "窗口" },
  { id: "hotkeys", label: "快捷键" },
  { id: "about", label: "关于" },
];

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export default function SettingsWindow() {
  const [tab, setTab] = useState<SettingsTab>("general");
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [baseUrl, setBaseUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [mascotId, setMascotId] = useState<MascotId>("peek");
  const [shortcutOpenPet, setShortcutOpenPet] = useState(
    DEFAULT_APP_CONFIG.shortcutOpenPet,
  );
  const [shortcutOpenHome, setShortcutOpenHome] = useState(
    DEFAULT_APP_CONFIG.shortcutOpenHome,
  );
  const [keepWindowsVisible, setKeepWindowsVisible] = useState(
    DEFAULT_APP_CONFIG.keepWindowsVisible,
  );
  const [notice, setNotice] = useState<Notice>(null);
  const [busy, setBusy] = useState<"save" | "test" | null>(null);
  const rootRef = useRef<HTMLElement | null>(null);

  useAutoFitWindow(rootRef, SETTINGS_WIDTH, [tab, notice, config]);
  useEscapeHidesWindow();

  useEffect(() => {
    let active = true;

    tauriApi
      .loadConfig()
      .then(async (loadedConfig) => {
        if (!active) return;
        setConfig(loadedConfig);
        setBaseUrl(loadedConfig.baseUrl);
        setUsername(loadedConfig.username);
        setMascotId(loadedConfig.mascotId);
        setShortcutOpenPet(
          loadedConfig.shortcutOpenPet || DEFAULT_APP_CONFIG.shortcutOpenPet,
        );
        setShortcutOpenHome(
          loadedConfig.shortcutOpenHome || DEFAULT_APP_CONFIG.shortcutOpenHome,
        );
        setKeepWindowsVisible(loadedConfig.keepWindowsVisible);
        if (!loadedConfig.username.trim()) return;

        const savedPassword = await tauriApi
          .getSecret("password")
          .catch(() => null);
        if (active) setPassword(savedPassword ?? "");
      })
      .catch((error: unknown) => {
        if (active) {
          setNotice({
            kind: "error",
            text: `加载设置失败：${errorMessage(error)}`,
          });
        }
      });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void tauriApi
      .listenMascotChanged((payload) => {
        setMascotId(payload);
        setConfig((current) =>
          current ? { ...current, mascotId: payload } : current,
        );
      })
      .then((registered) => {
        if (disposed) registered();
        else unlisten = registered;
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function chooseMascot(next: MascotId) {
    setMascotId(next);
    try {
      await tauriApi.patchConfig({ mascotId: next });
      setConfig((current) =>
        current ? { ...current, mascotId: next } : current,
      );
      await tauriApi.emitMascotChanged(next);
    } catch (error) {
      setNotice({
        kind: "error",
        text: `切换形象失败：${errorMessage(error)}`,
      });
    }
  }

  async function toggleKeepWindowsVisible(next: boolean) {
    const previous = keepWindowsVisible;
    setKeepWindowsVisible(next);
    try {
      await tauriApi.patchConfig({ keepWindowsVisible: next });
      await tauriApi.applyWindowDeactivatePolicy();
      setConfig((current) =>
        current ? { ...current, keepWindowsVisible: next } : current,
      );
    } catch (error) {
      setKeepWindowsVisible(previous);
      setNotice({
        kind: "error",
        text: `更新窗口设置失败：${errorMessage(error)}`,
      });
    }
  }

  async function persistCredentials(
    normalizedBaseUrl: string,
    extra: Partial<{
      shortcutOpenPet: string;
      shortcutOpenHome: string;
      keepWindowsVisible: boolean;
    }> = {},
  ) {
    const patch = {
      baseUrl: normalizedBaseUrl,
      username,
      mascotId,
      ...extra,
    };
    await tauriApi.patchConfig(patch);
    if (username.trim()) {
      await tauriApi.setSecret("password", password);
    }
    return patch;
  }

  async function saveSettings() {
    if (!config) return;

    setBusy("save");
    setNotice(null);
    try {
      const normalizedBaseUrl = normalizeBaseUrl(baseUrl);
      const patch = await persistCredentials(normalizedBaseUrl, {
        shortcutOpenPet:
          shortcutOpenPet.trim() || DEFAULT_APP_CONFIG.shortcutOpenPet,
        shortcutOpenHome:
          shortcutOpenHome.trim() || DEFAULT_APP_CONFIG.shortcutOpenHome,
        keepWindowsVisible,
      });
      await tauriApi.reloadHotkeys();
      await tauriApi.applyWindowDeactivatePolicy();
      const nextConfig = { ...config, ...patch };
      setConfig(nextConfig);
      setBaseUrl(normalizedBaseUrl);
      setShortcutOpenPet(
        patch.shortcutOpenPet || DEFAULT_APP_CONFIG.shortcutOpenPet,
      );
      setShortcutOpenHome(
        patch.shortcutOpenHome || DEFAULT_APP_CONFIG.shortcutOpenHome,
      );
      setKeepWindowsVisible(
        patch.keepWindowsVisible ?? DEFAULT_APP_CONFIG.keepWindowsVisible,
      );
      try {
        const { access_token } = await login(
          normalizedBaseUrl,
          username,
          password,
        );
        await tauriApi.setSecret("access_token", access_token);
        await tauriApi.emitAuthUpdated();
        await hideCurrentWindow();
      } catch (loginError) {
        await tauriApi.emitAuthUpdated();
        setNotice({
          kind: "error",
          text: `设置已保存，但连接失败：${errorMessage(loginError)}`,
        });
      }
    } catch (error) {
      setNotice({
        kind: "error",
        text: `保存失败：${errorMessage(error)}`,
      });
    } finally {
      setBusy(null);
    }
  }

  async function testConnection() {
    setBusy("test");
    setNotice(null);
    try {
      const normalizedBaseUrl = normalizeBaseUrl(baseUrl);
      const { access_token } = await login(
        normalizedBaseUrl,
        username,
        password,
      );
      const patch = await persistCredentials(normalizedBaseUrl);
      await tauriApi.setSecret("access_token", access_token);
      if (config) setConfig({ ...config, ...patch });
      setBaseUrl(normalizedBaseUrl);
      await tauriApi.emitAuthUpdated();
      setNotice({ kind: "success", text: "连接成功" });
    } catch (error) {
      setNotice({
        kind: "error",
        text: `连接失败：${errorMessage(error)}`,
      });
    } finally {
      setBusy(null);
    }
  }

  return (
    <main className="settings-window" ref={rootRef}>
      <nav className="settings-tabs-bar" data-tauri-drag-region>
        <div className="settings-tabs" role="tablist" aria-label="设置分类">
          {TABS.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={tab === item.id}
              className={`settings-tab${tab === item.id ? " is-active" : ""}`}
              onClick={() => {
                setTab(item.id);
                setNotice(null);
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
        <WindowCloseButton />
      </nav>

      <div className="settings-frame">
        {tab === "general" ? (
          <form
            className="settings-panel"
            onSubmit={(event) => {
              event.preventDefault();
              void saveSettings();
            }}
          >
            <div className="settings-subgroup">
              <div className="settings-subgroup-title">连接</div>
              <div className="settings-rows">
                <div className="settings-row">
                  <label htmlFor="base-url">服务地址</label>
                  <input
                    id="base-url"
                    type="text"
                    inputMode="url"
                    value={baseUrl}
                    onChange={(event) => setBaseUrl(event.currentTarget.value)}
                    placeholder="http://localhost:8088"
                    autoComplete="url"
                  />
                </div>
                <div className="settings-row">
                  <label htmlFor="username">用户</label>
                  <input
                    id="username"
                    value={username}
                    onChange={(event) => setUsername(event.currentTarget.value)}
                    autoComplete="username"
                  />
                </div>
                <div className="settings-row">
                  <label htmlFor="password">密码</label>
                  <input
                    id="password"
                    type="password"
                    value={password}
                    onChange={(event) => setPassword(event.currentTarget.value)}
                    autoComplete="current-password"
                  />
                </div>
              </div>
            </div>

            <div className="settings-subgroup">
              <div className="settings-subgroup-title">桌宠形象</div>
              <div
                className="mascot-picker"
                role="listbox"
                aria-label="选择形象"
              >
                {MASCOT_OPTIONS.map((option) => {
                  const selected = mascotId === option.id;
                  return (
                    <button
                      key={option.id}
                      type="button"
                      role="option"
                      aria-selected={selected}
                      className={`mascot-option${selected ? " is-selected" : ""}`}
                      aria-label={option.label}
                      onClick={() => void chooseMascot(option.id)}
                    >
                      <img
                        src={MASCOT_SRC[option.id]}
                        alt=""
                        draggable={false}
                      />
                      <span>{option.label}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            <div className="settings-footer">
              {notice ? (
                <p
                  className={`settings-notice settings-notice--${notice.kind}`}
                  role={notice.kind === "error" ? "alert" : "status"}
                  aria-live="polite"
                >
                  {notice.text}
                </p>
              ) : (
                <span />
              )}
              <div className="settings-footer-actions">
                <button
                  className="settings-btn"
                  type="button"
                  disabled={!config || busy !== null}
                  onClick={() => void testConnection()}
                >
                  {busy === "test" ? "测试中…" : "测试连接"}
                </button>
                <button
                  className="settings-btn"
                  type="submit"
                  disabled={!config || busy !== null}
                >
                  {busy === "save" ? "保存中…" : "保存"}
                </button>
              </div>
            </div>
          </form>
        ) : null}

        {tab === "window" ? (
          <div className="settings-panel">
            <label className="settings-check" htmlFor="keep-windows-visible">
              <input
                id="keep-windows-visible"
                type="checkbox"
                checked={keepWindowsVisible}
                disabled={!config || busy !== null}
                onChange={(event) =>
                  void toggleKeepWindowsVisible(event.currentTarget.checked)
                }
              />
              <span>点击其他应用时保持窗口显示</span>
            </label>
            <p className="settings-hint">
              关闭后，点到桌面或其他应用会隐藏聊天和设置。桌宠始终显示。
            </p>
            {notice ? (
              <p
                className={`settings-notice settings-notice--${notice.kind}`}
                role={notice.kind === "error" ? "alert" : "status"}
                aria-live="polite"
              >
                {notice.text}
              </p>
            ) : null}
          </div>
        ) : null}

        {tab === "hotkeys" ? (
          <div className="settings-panel">
            <div className="settings-rows">
              <div className="settings-row">
                <label htmlFor="shortcut-open-pet">打开桌宠</label>
                <ShortcutRecorder
                  id="shortcut-open-pet"
                  value={shortcutOpenPet}
                  onChange={setShortcutOpenPet}
                  disabled={!config || busy !== null}
                />
              </div>
              <div className="settings-row">
                <label htmlFor="shortcut-open-home">打开服务主页</label>
                <ShortcutRecorder
                  id="shortcut-open-home"
                  value={shortcutOpenHome}
                  onChange={setShortcutOpenHome}
                  disabled={!config || busy !== null}
                />
              </div>
            </div>
            <p className="settings-hint">
              点击后按下组合键即可录制；按 Esc 取消，Backspace 清除。
            </p>
            <div className="settings-footer">
              {notice ? (
                <p
                  className={`settings-notice settings-notice--${notice.kind}`}
                  role={notice.kind === "error" ? "alert" : "status"}
                  aria-live="polite"
                >
                  {notice.text}
                </p>
              ) : (
                <span />
              )}
              <div className="settings-footer-actions">
                <button
                  className="settings-btn"
                  type="button"
                  disabled={!config || busy !== null}
                  onClick={() => void saveSettings()}
                >
                  {busy === "save" ? "保存中…" : "保存快捷键"}
                </button>
              </div>
            </div>
          </div>
        ) : null}

        {tab === "about" ? (
          <div className="settings-panel">
            <div className="settings-rows settings-about">
              <div className="settings-row">
                <span>应用</span>
                <span>{APP_NAME}</span>
              </div>
              <div className="settings-row">
                <span>版本</span>
                <span>V{appVersion}</span>
              </div>
            </div>
            {notice ? (
              <p
                className={`settings-notice settings-notice--${notice.kind}`}
                role={notice.kind === "error" ? "alert" : "status"}
                aria-live="polite"
              >
                {notice.text}
              </p>
            ) : null}
          </div>
        ) : null}
      </div>
    </main>
  );
}
