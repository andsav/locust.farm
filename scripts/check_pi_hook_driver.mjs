// Fake Pi API replay of the actual setup-generated extension. No Pi package,
// model, profile discovery, or Locust decisions are implemented in this driver.
import { readFile, realpath } from "node:fs/promises";
import { readFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import { isAbsolute, relative } from "node:path";
import { fileURLToPath } from "node:url";

function requireValue(condition) {
    if (!condition) throw new Error("Invalid fake Pi replay");
}

function oneLine(values) {
    requireValue(values.length <= 1);
    const line = values[0];
    if (line === undefined) return undefined;
    requireValue(typeof line === "string" && line.length > 0 && line.length < 512 && /^[\x20-\x7e]+$/.test(line));
    return line;
}

export async function replay(extension, frame) {
    const home = await realpath(process.env.HOME);
    const source = await realpath(extension);
    const fromHome = relative(home, source);
    requireValue(fromHome !== "" && !fromHome.startsWith("..") && !isAbsolute(fromHome));
    requireValue(typeof frame.session_id === "string" && frame.session_id !== "");
    const event = frame.event;
    requireValue(event !== null && typeof event === "object" && !Array.isArray(event));
    const handlers = new Map();
    const messages = [];
    const pi = {
        on(name, handler) {
            requireValue(typeof name === "string" && typeof handler === "function" && !handlers.has(name));
            handlers.set(name, handler);
        },
        sendMessage(message, options) {
            requireValue(message.customType === "locust" && options.triggerTurn === false);
            messages.push(message.content);
        },
    };
    const controller = new AbortController();
    // Pi's run mode reaches the launcher; replays default to a person's TUI chat.
    requireValue(frame.mode === undefined || typeof frame.mode === "string");
    const ctx = { sessionManager: { getSessionId: () => frame.session_id }, signal: controller.signal, mode: frame.mode ?? "tui" };
    const text = await readFile(source, "utf8");
    const javascript = stripTypeScriptTypes(text, { mode: "strip" });
    const module = await import("data:text/javascript;base64," + Buffer.from(javascript).toString("base64"));
    requireValue(typeof module.default === "function");
    await module.default(pi);
    const handler = handlers.get(event.type);
    requireValue(typeof handler === "function");
    const original = JSON.parse(JSON.stringify(event));
    let returned;
    try {
        returned = await handler(event, ctx);
    } finally {
        await handlers.get("session_shutdown")?.({ type: "session_shutdown" }, ctx);
    }
    let line;
    let keepGoing = false;
    let toolResultPreserved = JSON.stringify(event) === JSON.stringify(original);
    if (event.type === "tool_result") {
        requireValue(messages.length === 0);
        if (returned !== undefined) {
            requireValue(Array.isArray(returned.content));
            toolResultPreserved = toolResultPreserved && JSON.stringify(returned.content.slice(0, original.content.length)) === JSON.stringify(original.content)
                && JSON.stringify(returned.details) === JSON.stringify(original.details)
                && JSON.stringify(returned.structuredContent) === JSON.stringify(original.structuredContent)
                && returned.isError === original.isError
                && JSON.stringify(returned.usage) === JSON.stringify(original.usage);
            line = oneLine(returned.content.slice(original.content.length).map((item) => {
                requireValue(item.type === "text");
                return item.text;
            }));
        }
    } else if (event.type === "agent_before_settle") {
        requireValue(messages.length === 0);
        if (returned !== undefined) {
            requireValue(Array.isArray(returned.entries)
                && JSON.stringify(returned.entries.slice(0, original.entries.length)) === JSON.stringify(original.entries)
                && typeof returned.continue === "boolean");
            line = oneLine(returned.entries.slice(original.entries.length).map((entry) => {
                requireValue(entry.type === "custom_message" && entry.customType === "locust");
                return entry.content;
            }));
            keepGoing = returned.continue;
        }
    } else {
        requireValue(returned === undefined);
        line = oneLine(messages);
    }
    requireValue(toolResultPreserved);
    // This is comparison transport only: Pi's actual native continuation flag
    // and extension-authored line determine the projected generic envelope.
    const envelope = line === undefined ? null : keepGoing
        ? { decision: "block", reason: line }
        : { hookSpecificOutput: { hookEventName: event.type, additionalContext: line } };
    return { envelope, through_installed_extension: true, tool_result_preserved: toolResultPreserved };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
    try {
        const frame = JSON.parse(readFileSync(0, "utf8"));
        const result = await replay(process.argv[2], frame);
        process.stdout.write(JSON.stringify(result) + "\n");
    } catch {
        process.stderr.write("Fake Pi replay failed\n");
        process.exitCode = 1;
    }
}
