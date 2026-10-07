import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  createPdChatBridge,
  PD_CHAT_PROTOCOL,
  supportsPdChatBridge,
  isPdCliArgs,
  pdCliErrorMessage,
} from "./pdChatBridge";
import { tauriApi } from "./tauriApi";
vi.mock("./tauriApi", () => ({ tauriApi: { pdExecuteCli: vi.fn() } }));
const request = {
  type: "pd_cli_request",
  protocol: PD_CHAT_PROTOCOL,
  id: "a".repeat(32),
  thread_id: "thread",
  method: "helper.cli",
  args: ["cli", "window", "list-visible"],
};
function setup(url = "wss://example.test/chat") {
  const socket = {
    url,
    readyState: WebSocket.OPEN,
    send: vi.fn(),
  } as unknown as WebSocket;
  return {
    socket,
    handle: createPdChatBridge(socket, "thread"),
  };
}
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(tauriApi.pdExecuteCli).mockResolvedValue({
    ok: true,
    windows: [{ title: "测试" }],
  });
});
describe("PD bridge on the existing chat socket", () => {
  it("read-only probes run without confirmation", async () => {
    const { handle } = setup();
    for (const [index, args] of [
      ["cli", "status"],
      ["cli", "describe"],
      ["cli", "capabilities"],
      ["cli", "access", "list"],
      ["cli", "window", "list-visible"],
    ].entries())
      handle({ ...request, id: index.toString(16).padStart(32, "0"), args });
    await vi.waitFor(() =>
      expect(tauriApi.pdExecuteCli).toHaveBeenCalledTimes(5),
    );
  });
  it("reports unavailable helper errors in chat and returns an error frame", async () => {
    vi.mocked(tauriApi.pdExecuteCli).mockRejectedValue(
      "未找到已安装的小助手，请先安装 PD 小助手",
    );
    const { socket } = setup();
    const report = vi.fn();
    createPdChatBridge(socket, "thread", report)(request);
    await vi.waitFor(() =>
      expect(report).toHaveBeenCalledWith(
        "未找到已安装的小助手，请先安装 PD 小助手",
      ),
    );
    expect(
      JSON.parse(vi.mocked(socket.send).mock.calls[0][0] as string).result.ok,
    ).toBe(false);
  });
  it("idle status is normal; CLI errors preserve useful reasons", () => {
    expect(
      pdCliErrorMessage({
        ok: true,
        exit_code: 0,
        stdout: '{"running":false,"status":"idle"}',
      }),
    ).toBe("");
    expect(
      pdCliErrorMessage({
        ok: true,
        exit_code: 1,
        stdout: '{"ok":false,"error":"小助手连接不可用"}',
        stderr: "",
      }),
    ).toContain("小助手连接不可用");
  });

  it("ordinary chat chunks never invoke the local helper", () => {
    const { handle } = setup();
    expect(handle({ type: "token", content: "normal chat" })).toBe(false);
    expect(tauriApi.pdExecuteCli).not.toHaveBeenCalled();
  });
  it.each([
    ["cli", "window", "activate", "--handle", "123"],
    ["cli", "mouse", "click", "--x", "10", "--y", "20"],
    ["cli", "keyboard", "type", "text"],
    ["cli", "screen", "capture"],
    ["cli", "browser", "status"],
    ["browser", "status"],
    ["browser", "open", "chrome", "--url", "https://example.test"],
    [
      "cli",
      "browser",
      "navigate",
      "--browser",
      "chrome",
      "https://example.test",
    ],
  ])(
    "executes supported CLI arguments without an approval callback: %s",
    async (...args) => {
      const { socket, handle } = setup();
      handle({ ...request, args });
      await vi.waitFor(() => expect(socket.send).toHaveBeenCalled());
      expect(tauriApi.pdExecuteCli).toHaveBeenCalledWith(args);
    },
  );
  it("rejects operating-system commands and disabled CLI families", () => {
    for (const args of [
      ["bash", "-c", "echo test"],
      ["cli", "workflow", "run"],
      ["cli", "access", "activate"],
      ["cli", "screen", "bad\0"],
    ])
      expect(isPdCliArgs(args)).toBe(false);
    expect(isPdCliArgs(["cli", "status"])).toBe(true);
    expect(isPdCliArgs(["cli", "screen", "capture"])).toBe(true);
  });

  it("returns the CLI result on the exact same socket and executes duplicates once", async () => {
    const { socket, handle } = setup();
    handle(request);
    handle(request);
    await vi.waitFor(() => expect(socket.send).toHaveBeenCalled());
    expect(tauriApi.pdExecuteCli).toHaveBeenCalledTimes(1);
    expect(
      JSON.parse(vi.mocked(socket.send).mock.calls[0][0] as string),
    ).toEqual({
      ...request,
      type: "pd_cli_result",
      method: undefined,
      args: undefined,
      result: { ok: true, windows: [{ title: "测试" }] },
    });
  });
  it("ignores another thread or protocol", () => {
    const { handle } = setup();
    handle({ ...request, thread_id: "other" });
    handle({ ...request, protocol: "other" });
    expect(tauriApi.pdExecuteCli).not.toHaveBeenCalled();
  });
  it("rejects arbitrary operations on the existing HTTP chat connection", async () => {
    const { socket, handle } = setup("ws://47.77.184.54:8088/chat");
    handle({ ...request, method: "mouse.click" });
    await vi.waitFor(() => expect(socket.send).toHaveBeenCalled());
    expect(tauriApi.pdExecuteCli).not.toHaveBeenCalled();
  });
  it("executes a CLI request over the same remote WS connection", async () => {
    const { socket, handle } = setup("ws://47.77.184.54:8088/chat");
    handle(request);
    await vi.waitFor(() => expect(socket.send).toHaveBeenCalled());
    expect(tauriApi.pdExecuteCli).toHaveBeenCalledWith(request.args);
    expect(
      JSON.parse(vi.mocked(socket.send).mock.calls[0][0] as string).type,
    ).toBe("pd_cli_result");
  });
  it("does not return a late result to a closed connection", async () => {
    const { socket, handle } = setup();
    handle(request);
    Object.assign(socket, { readyState: WebSocket.CLOSED });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(socket.send).not.toHaveBeenCalled();
  });
  it("permits existing WS and future WSS connections", () => {
    expect(supportsPdChatBridge("ws://127.0.0.1:8088/chat")).toBe(true);
    expect(supportsPdChatBridge("ws://47.77.184.54:8088/chat")).toBe(true);
    expect(supportsPdChatBridge("https://example.test/chat")).toBe(false);
    expect(supportsPdChatBridge("wss://example.test/chat")).toBe(true);
  });
});
