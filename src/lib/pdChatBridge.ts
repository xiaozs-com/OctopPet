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
  const activeRequests = new Set<string>();
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
      if (activeRequests.size >= 8) {
        result = Promise.resolve({
          ok: false,
          error: "本机 CLI 请求队列繁忙，请稍后重试",
        });
      } else {
        activeRequests.add(id);
        result = (async () => {
          if (
            !supportsPdChatBridge(socket.url) ||
            !["helper.cli", "helper.cli.io"].includes(String(frame.method)) ||
            !isPdCliArgs(frame.args)
          )
            return { ok: false, error: "此连接或操作不支持本机桥接" };
          try {
            if (socket.readyState !== WebSocket.OPEN)
              return { ok: false, error: "聊天连接已结束" };
            if (frame.method === "helper.cli.io") {
              if (!isPdCliInput(frame.input))
                return { ok: false, error: "CLI 标准输入或文件传输参数无效" };
              return await tauriApi.pdExecuteCli(
                frame.args as string[],
                frame.input,
              );
            }
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
        })().finally(() => {
          activeRequests.delete(id);
          for (const key of requests.keys()) {
            if (requests.size <= 128) break;
            if (!activeRequests.has(key)) requests.delete(key);
          }
        });
      }
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
    value.length < 1 ||
    value.length > 128 ||
    !["cli", "browser"].includes(value[0]) ||
    value.some(
      (arg) =>
        typeof arg !== "string" || arg.length > 4096 || arg.includes("\0"),
    )
  )
    return false;
  return true;
}

export function isPdCliInput(value: unknown): boolean {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const input = value as Record<string, unknown>;
  if (
    Object.keys(input).some(
      (key) => !["stdin", "uploads", "downloads"].includes(key),
    )
  )
    return false;
  if (
    input.stdin !== undefined &&
    (typeof input.stdin !== "string" || input.stdin.length > 1024 * 1024)
  )
    return false;
  for (const key of ["uploads", "downloads"]) {
    if (
      input[key] !== undefined &&
      (!Array.isArray(input[key]) || input[key].length > 128)
    )
      return false;
  }
  return JSON.stringify(input).length <= 8 * 1024 * 1024;
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
