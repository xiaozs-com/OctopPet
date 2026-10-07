import { tauriApi } from "./tauriApi";

export const PD_CHAT_PROTOCOL = "pd-chat-cli@2";

export function supportsPdChatBridge(url: string): boolean {
  const parsed = new URL(url);
  return parsed.protocol === "ws:" || parsed.protocol === "wss:";
}

export function createPdChatBridge(
  socket: WebSocket,
  threadId: string,
  reportError: (message: string) => void = () => {},
) {
  const requests = new Map<string, Promise<unknown>>();
  return (chunk: unknown): boolean => {
    if (!chunk || typeof chunk !== "object") return false;
    const frame = chunk as Record<string, unknown>;
    if (frame.type !== "pd_cli_request") return false;
    if (
      frame.protocol !== PD_CHAT_PROTOCOL ||
      frame.thread_id !== threadId ||
      typeof frame.id !== "string" ||
      !/^[a-f0-9]{32}$/.test(frame.id)
    )
      return true;
    const id = frame.id;
    let result = requests.get(id);
    if (!result) {
      if (requests.size >= 8) return true;
      result = (async () => {
        if (
          !supportsPdChatBridge(socket.url) ||
          frame.method !== "helper.cli" ||
          !isPdCliArgs(frame.args)
        )
          return { ok: false, error: "此连接或操作不支持本机桥接" };
        try {
          if (socket.readyState !== WebSocket.OPEN)
            return { ok: false, error: "聊天连接已结束" };
          return await tauriApi.pdExecuteCli(frame.args as string[]);
        } catch (error) {
          const reason =
            error instanceof Error
              ? error.message
              : typeof error === "string"
                ? error
                : "小助手调用失败，请检查本机安装和运行状态";
          return { ok: false, error: reason };
        }
      })();
      requests.set(id, result);
    }
    void result.then((output) => {
      const message = pdCliErrorMessage(output);
      if (socket.readyState === WebSocket.OPEN && message) reportError(message);
      if (socket.readyState === WebSocket.OPEN)
        socket.send(
          JSON.stringify({
            type: "pd_cli_result",
            protocol: PD_CHAT_PROTOCOL,
            id,
            thread_id: threadId,
            result: output,
          }),
        );
    });
    return true;
  };
}

export function isPdCliArgs(value: unknown): value is string[] {
  if (
    !Array.isArray(value) ||
    value.length < 2 ||
    value.length > 128 ||
    !["cli", "browser"].includes(value[0]) ||
    value.some(
      (arg) =>
        typeof arg !== "string" || arg.length > 4096 || arg.includes("\0"),
    )
  )
    return false;
  if (value[0] === "browser") return true;
  const direct = ["status", "describe", "capabilities", "health"];
  if (direct.includes(value[1])) return value.length === 2;
  if (value[1] === "access") return value.length === 3 && value[2] === "list";
  return (
    ["window", "screen", "mouse", "keyboard", "ocr", "browser"].includes(
      value[1],
    ) && value.length >= 3
  );
}

export function pdCliErrorMessage(output: unknown): string {
  if (!output || typeof output !== "object") return "";
  const value = output as Record<string, unknown>;
  if (value.ok === false)
    return typeof value.error === "string"
      ? value.error.slice(0, 600)
      : "小助手调用失败，请检查本机安装和运行状态";
  if (typeof value.exit_code === "number" && value.exit_code !== 0) {
    let detail = typeof value.stderr === "string" ? value.stderr.trim() : "";
    if (typeof value.stdout === "string") {
      try {
        const parsed = JSON.parse(value.stdout);
        if (typeof parsed.error === "string") detail = parsed.error;
      } catch {
        /* CLI output may be plain text. */
      }
    }
    return (
      "小助手执行失败：" + (detail.slice(0, 600) || `退出码 ${value.exit_code}`)
    );
  }
  return "";
}
