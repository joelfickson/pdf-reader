import json
import sys


def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()


def update(session, body):
    send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": session, "update": body}})


mcp = ""
for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"protocolVersion": 1, "agentCapabilities": {}, "agentInfo": {"name": "fake", "title": "Fake Agent", "version": "1"}}})
    elif method == "session/new":
        server = message["params"]["mcpServers"][0]
        mcp = f"{server['name']} {server['command']} {' '.join(server['args'])}"
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"sessionId": "s1"}})
    elif method == "session/prompt":
        prompt = message["params"]["prompt"]
        link = next(block["uri"] for block in prompt if block["type"] == "resource_link")
        update("s1", {"sessionUpdate": "plan", "entries": [{"content": "Read the outline", "priority": "medium", "status": "in_progress"}]})
        update("s1", {"sessionUpdate": "tool_call", "toolCallId": "t1", "title": "mcp__agenticpdf__get_text", "kind": "read", "status": "pending", "rawInput": {"from_page": 1}})
        update("s1", {"sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed"})
        send({"jsonrpc": "2.0", "id": 900, "method": "session/request_permission", "params": {"sessionId": "s1", "toolCall": {"toolCallId": "t2", "title": "mcp__agenticpdf__highlight"}, "options": [{"optionId": "yes", "name": "Allow once", "kind": "allow_once"}, {"optionId": "no", "name": "Reject", "kind": "reject_once"}]}})
        while True:
            reply = json.loads(sys.stdin.readline())
            if reply.get("id") == 900:
                break
        outcome = reply["result"]["outcome"]
        chosen = outcome.get("optionId", outcome["outcome"])
        update("s1", {"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": f"Chose {chosen} for {link} "}})
        update("s1", {"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": f"via {mcp} (p. 1)."}})
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
