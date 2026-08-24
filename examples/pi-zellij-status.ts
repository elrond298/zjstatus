import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { chmod, mkdir, rename, unlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

interface Status {
  pid: number;
  paneId?: string;
  sessionId?: string;
  sessionName?: string;
  cwd?: string;
  model?: string;
  thinking?: string;
  busy: boolean;
  tool?: string;
  goal?: string;
  todo?: string;
  subagents: string[];
  updatedAt: number;
}

const zellijSession = process.env.ZELLIJ_SESSION_NAME;

export default function (pi: ExtensionAPI) {
  if (!zellijSession) return;

  const uid = process.getuid?.() ?? 0;
  const root = join(process.env.XDG_RUNTIME_DIR || tmpdir(), `pi-zellij-status-${uid}`);
  const dir = join(root, zellijSession.replace(/[^A-Za-z0-9_.-]/g, "_"));
  const file = join(dir, `${process.pid}.json`);
  const temp = `${file}.tmp`;
  const todos = new Map<string, string>();
  const subagents = new Map<string, string>();
  const pending = new Map<string, { name: string; args: any }>();
  const status: Status = {
    pid: process.pid,
    paneId: process.env.ZELLIJ_PANE_ID,
    busy: false,
    subagents: [],
    updatedAt: Date.now(),
  };
  let writes = Promise.resolve();

  const publish = () => {
    status.todo = Array.from(todos.values()).at(-1);
    status.subagents = Array.from(subagents.values());
    status.updatedAt = Date.now();
    const json = JSON.stringify(status);
    writes = writes
      .then(async () => {
        await mkdir(dir, { recursive: true, mode: 0o700 });
        await chmod(root, 0o700);
        await writeFile(temp, json, { mode: 0o600 });
        await rename(temp, file);
      })
      .catch(() => {});
  };

  const applyTodo = (name: string, args: any) => {
    if (name === "todowrite" && Array.isArray(args?.todos)) {
      todos.clear();
      for (const item of args.todos) {
        if (item?.status === "in_progress") {
          todos.set(String(item.id ?? "current"), item.activeForm ?? item.content ?? "working");
        }
      }
      return;
    }
    if (name !== "todo" || args?.action !== "update" || args?.id == null) return;
    const id = String(args.id);
    if (["completed", "deleted"].includes(args.status)) todos.delete(id);
    else if (args.status === "in_progress") {
      todos.set(id, args.activeForm ?? args.subject ?? `todo ${id}`);
    } else if (todos.has(id) && args.activeForm) {
      todos.set(id, args.activeForm);
    }
  };

  const readText = (message: any) => {
    if (typeof message?.content === "string") return message.content;
    if (!Array.isArray(message?.content)) return "";
    return message.content
      .filter((part: any) => part?.type === "text")
      .map((part: any) => part.text)
      .join("\n");
  };

  const restore = (ctx: ExtensionContext) => {
    todos.clear();
    subagents.clear();
    for (const entry of ctx.sessionManager.getBranch() as any[]) {
      if (entry.type === "custom" && entry.customType === "goal-state") {
        const goal = entry.data?.goal;
        status.goal = goal && goal.status !== "complete" ? goal.text : undefined;
      }
      if (entry.type === "custom" && entry.customType === "subagents:record") {
        const agent = entry.data;
        if (agent?.status === "running" || agent?.status === "background") {
          subagents.set(agent.id, agent.description ?? agent.type ?? agent.id);
        } else if (agent?.id) subagents.delete(agent.id);
      }
      if (entry.type !== "message") continue;
      const message = entry.message;
      if (message?.role === "assistant" && Array.isArray(message.content)) {
        for (const part of message.content) {
          if (part?.type === "toolCall") applyTodo(part.name, part.arguments);
        }
      }
      if (message?.role === "toolResult" && message.toolName === "Agent") {
        const details = message.details;
        if (details?.status === "background" && details.agentId) {
          subagents.set(details.agentId, details.description ?? details.displayName ?? details.agentId);
        }
      }
    }
  };

  pi.on("session_start", (_event, ctx) => {
    restore(ctx);
    status.sessionId = ctx.sessionManager.getSessionId();
    status.sessionName = pi.getSessionName();
    status.cwd = ctx.cwd;
    status.model = ctx.model?.id;
    status.thinking = ctx.thinkingLevel;
    status.busy = !ctx.isIdle();
    publish();
  });

  pi.on("session_info_changed", (event) => {
    status.sessionName = event.name;
    publish();
  });

  pi.on("before_agent_start", (_event, ctx) => {
    restore(ctx);
    publish();
  });

  pi.on("model_select", (event, ctx) => {
    status.model = event.model.id;
    status.thinking = ctx.thinkingLevel;
    publish();
  });

  pi.on("thinking_level_select", (event) => {
    status.thinking = event.level;
    publish();
  });

  pi.on("agent_start", () => {
    status.busy = true;
    publish();
  });

  pi.on("agent_settled", () => {
    status.busy = false;
    status.tool = undefined;
    publish();
  });

  pi.on("tool_execution_start", (event) => {
    pending.set(event.toolCallId, { name: event.toolName, args: event.args });
    status.tool = event.toolName;
    publish();
  });

  pi.on("tool_execution_end", (event) => {
    const call = pending.get(event.toolCallId);
    pending.delete(event.toolCallId);
    status.tool = Array.from(pending.values()).at(-1)?.name;
    if (!event.isError && call) applyTodo(call.name, call.args);

    if (!event.isError && event.toolName === "Agent") {
      const details = event.result?.details;
      if (details?.status === "background" && details.agentId) {
        subagents.set(details.agentId, details.description ?? details.displayName ?? details.agentId);
      }
    }
    if (!event.isError && event.toolName === "goal_complete") status.goal = undefined;
    publish();
  });

  pi.on("message_start", (event) => {
    const message = event.message as any;
    const text = readText(message);
    const objective = text.match(/<goal_objective>\s*([\s\S]*?)\s*<\/goal_objective>/)?.[1];
    if (objective) status.goal = objective.trim();

    if (message.customType === "subagent-notification") {
      const id = message.details?.id ?? text.match(/<task-id>(.*?)<\/task-id>/)?.[1];
      if (id) subagents.delete(id);
    }
    publish();
  });

  pi.on("session_shutdown", async () => {
    await writes;
    await Promise.allSettled([unlink(file), unlink(temp)]);
  });
}
