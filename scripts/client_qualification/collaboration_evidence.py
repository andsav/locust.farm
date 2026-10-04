"""Independent evidence checks for the shared-context real-model fixture."""

import json
import os
from pathlib import Path
import subprocess
import tempfile

from client_qualification import collaboration_case
from client_qualification.runtime import records


def _calls(events, name):
    requests = {event.get("id"): event for event in events
                if event.get("direction") == "client_request" and event.get("method") == "tools/call"
                and event.get("tool") == name}
    result = []
    for event in events:
        if event.get("direction") != "bridge_response" or event.get("tool") != name:
            continue
        request = requests.get(event.get("id"))
        envelope = event.get("result", {})
        if request and event.get("isError") is False and envelope.get("ok") is True:
            result.append((request, envelope.get("result", {})))
    return result


def _review(report, output):
    reviews = report.get("patch_reviews") or []
    value = reviews[0] if reviews else None
    if value is None:
        try:
            value = json.loads((output / "independent-patch-review.json").read_text())
        except (OSError, ValueError):
            return {}
    return value.get("result", value) if isinstance(value, dict) else {}


def _patch_matches(review, artifact, base):
    if not artifact.is_file() or artifact.is_symlink() or review.get("base") != base:
        return False
    changes = review.get("changes", [])
    if len(changes) != 1 or changes[0].get("path") != "safe_member.py":
        return False
    diff = changes[0].get("unified_diff", "")
    if ("--- a/safe_member.py\n+++ b/safe_member.py\n" not in diff
            or changes[0].get("after", {}).get("bytes") != artifact.stat().st_size):
        return False
    with tempfile.TemporaryDirectory(prefix="locust-collab-review-") as directory:
        root = Path(directory)
        (root / "safe_member.py").write_text(collaboration_case.STARTER, encoding="utf-8")
        env = {"PATH": os.defpath, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}
        for args in (["git", "apply", "--check", "--"], ["git", "apply", "--"]):
            applied = subprocess.run(args, cwd=root, env=env, input=diff, text=True,
                                     capture_output=True, check=False)
            if applied.returncode:
                return False
        return (root / "safe_member.py").read_bytes() == artifact.read_bytes()


def evaluate(report):
    runs, people = report.get("runs", []), report.get("principals", {})
    if len(runs) < 2:
        return {"two_model_runs_present": False}
    researcher, builder = people.get("researcher", {}), people.get("builder", {})
    output = Path(runs[1]["mcp_events"]).resolve().parent.parent

    findings = []
    for item in report.get("research_findings", []):
        event = item.get("event", {})
        body = event.get("body", {}).get("contribution_published", {})
        view = event.get("view", {})
        text = (event.get("text") or "").casefold()
        if (view.get("author") == researcher.get("principal")
                and view.get("kind") == "contribution_published" and view.get("standing") == "effective"
                and event.get("task") is None
                and body.get("attempt") is None and body.get("context", {}).get("scope") == "goal"
                and all(word in text for word in ("windows", "backslash", "linux", "normalize", ".."))):
            findings.append(item)
    finding_ids = {item["event"]["view"]["event"] for item in findings}

    finding_texts = {item["event"]["view"]["event"]: item["event"]["text"] for item in findings}
    events = records(runs[1].get("mcp_events"), strict=True)
    delivered = []
    for _, response in _calls(events, "locust_context_read"):
        context = response.get("context", {})
        receipt = context.get("receipt") or {}
        for item in context.get("items", []):
            event_id = item.get("event", {}).get("view", {}).get("event")
            seen = [entry for entry in receipt.get("entries", []) if entry.get("event") == event_id]
            if (event_id in finding_ids and item.get("text_complete") is True and seen
                    and item.get("event", {}).get("text") == finding_texts[event_id]
                    and all(entry.get("version") for entry in seen)):
                delivered.append((event_id, receipt))

    acknowledged = set()
    for request, response in _calls(events, "locust_context_acknowledge"):
        supplied = request.get("arguments", {}).get("receipt")
        accepted = response.get("context_acknowledged", {})
        for event_id, receipt in delivered:
            if (supplied == receipt and receipt.get("goal") == report.get("goal")
                    and receipt.get("principal") == builder.get("principal")
                    and receipt.get("session") == builder.get("instance")
                    and accepted.get("goal") == receipt.get("goal")
                    and accepted.get("principal") == receipt.get("principal")
                    and accepted.get("session") == receipt.get("session")
                    and accepted.get("entries") == receipt.get("entries")
                    and any(entry.get("event") == event_id for entry in receipt.get("entries", []))):
                acknowledged.add(event_id)

    final = report.get("final_context", [])
    contributions = [item for item in final
        if item.get("event", {}).get("view", {}).get("author") == builder.get("principal")
        and item.get("event", {}).get("view", {}).get("kind") == "contribution_published"]
    starts = {item.get("event", {}).get("view", {}).get("event") for item in final
              if item.get("event", {}).get("view", {}).get("author") == builder.get("principal")
              and item.get("event", {}).get("view", {}).get("kind") == "attempt_started"}
    attempted = [item for item in contributions
        if item.get("event", {}).get("body", {}).get("contribution_published", {}).get("attempt") in starts
        and item.get("event", {}).get("task")]
    opened = {item.get("event", {}).get("task"): item
              for item in final if item.get("event", {}).get("view", {}).get("kind") == "task_opened"}
    task_citations = [item for item in opened.values()
                      if any(event_id in (item.get("event", {}).get("text") or "") for event_id in finding_ids)]
    # The actual contribution's summary can omit a literal citation even when
    # its signed task-opening text names the finding; retain both facts.
    direct_citations = [item for item in attempted
        if any(event_id in (item.get("event", {}).get("text") or "") for event_id in finding_ids)]
    attributed = [item for item in attempted
        if item.get("event", {}).get("task") in opened
        and any(event_id in (opened[item["event"]["task"]].get("event", {}).get("text") or "")
                for event_id in finding_ids)]

    review = _review(report, output)
    artifact = Path(report.get("artifact_path", output / "builder-artifact/safe_member.py"))
    linked = any(
        (body := item.get("event", {}).get("body", {}).get("contribution_published", {})).get("patch")
            == review.get("contribution_id")
        and body.get("base") == review.get("base") == report.get("base")
        and review.get("head") in body.get("artifacts", [])
        for item in attempted)
    signed_event = next((item.get("event", {}).get("view", {}).get("event") for item in attempted
        if item.get("event", {}).get("body", {}).get("contribution_published", {}).get("patch")
        == review.get("contribution_id")), "")
    native_submit = any(call.get("success") is True
        and "patch submit" in call.get("arguments", {}).get("command", "")
        and review.get("contribution_id", "") in call.get("arguments", {}).get("command", "")
        and signed_event in str(call.get("output", "")) for call in runs[1].get("native_calls", []))

    control_items = report.get("control_context", [])
    control_unread = any(item.get("event", {}).get("view", {}).get("event") in finding_ids
                         and item.get("acknowledged") is False for item in control_items)
    control_unread &= report.get("control_pending", {}).get("pending", {}).get("context_news", {}).get("unacknowledged", 0) > 0

    research_response = runs[0].get("response", {})
    research_tokens = research_response.get("totals", {}).get("output_tokens", 0)
    build_events = records(runs[1].get("stdout"))
    assertions = {
        "different_principals": bool(researcher.get("principal") and builder.get("principal")
                                     and researcher["principal"] != builder["principal"]),
        "different_sessions": bool(researcher.get("instance") and builder.get("instance")
                                   and researcher["instance"] != builder["instance"]),
        "researcher_signed_goal_wide_finding_contains_rule": bool(findings),
        "builder_read_finding_with_exact_receipt": bool(delivered),
        "builder_acknowledged_exact_receipt": bool(acknowledged),
        "control_session_kept_finding_unread": control_unread,
        "task_opening_cites_finding_event": bool(task_citations),
        "contribution_is_bound_to_citing_task_and_attempt": bool(attributed),
        "direct_contribution_summary_cites_finding_event": bool(direct_citations),
        "native_patch_submit_matches_signed_contribution": linked and native_submit,
        "independent_review_reconstructs_saved_artifact": _patch_matches(review, artifact, report.get("base")),
        "hidden_oracle_passes": report.get("verification", {}).get("passed") is True,
        "starter_fails_oracle": report.get("baseline", {}).get("passed") is False,
        "researcher_real_completion_with_usage": runs[0].get("exit_code") == 0
            and research_response.get("status") == "completed" and research_tokens > 0,
        "builder_real_completion_with_usage": runs[1].get("exit_code") == 0 and any(event.get("type") == "turn.completed"
            and event.get("usage", {}).get("output_tokens", 0) > 0 for event in build_events),
        "git_head_preserved": report.get("git_head_preserved") is True,
        "unrelated_file_preserved": report.get("unrelated_preserved") is True,
        "readme_preserved": report.get("readme_preserved") is True,
        "builder_pending_empty": report.get("builder_pending", {}).get("pending", {}).get("context_news", {}).get("unacknowledged") == 0,
        "model_process_cleanup": all(run.get("exit_code") == 0 and run.get("natural_cleanup") is True
            and run.get("cleanup_verified") is True for run in runs),
    }
    ack_ids = {event.get("id") for event in events if event.get("direction") == "bridge_response"
               and event.get("tool") == "locust_context_acknowledge"
               and event.get("isError") is False and event.get("result", {}).get("ok") is True}
    ack_requests = [event for event in events if event.get("direction") == "client_request"
                    and event.get("tool") == "locust_context_acknowledge"]
    report["evidence_notes"] = {
        "acknowledgement_attempts": len(ack_requests),
        "rejected_acknowledgements": sum(request.get("id") not in ack_ids for request in ack_requests),
        "complete_finding_receipts": len(delivered), "exact_finding_acknowledgements": len(acknowledged),
        "direct_contribution_citations": len(direct_citations), "task_event_citations": len(task_citations),
        "builder_contributions": len(contributions)}
    return assertions
