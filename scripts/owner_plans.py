"""Repeat a reviewed owner command with its exact plan confirmation.

Qualification fixtures may approve their own synthetic setup. Person-facing
guides should instead show the plan and wait for the person's decision.
"""

import re


PLAN_ID = re.compile(r"plan-[0-9a-f]{16}\Z")


def confirmation_arguments(arguments, result):
    """Return the matching confirmation argv, or None for an immediate result."""
    if not isinstance(result, dict):
        return None
    if result.get("action") != "review_required":
        return None
    plan_id = result.get("plan_id")
    if not isinstance(plan_id, str) or not PLAN_ID.fullmatch(plan_id):
        raise ValueError("owner command returned an invalid plan id")
    argv = list(map(str, arguments))
    if "--confirm" in argv:
        raise ValueError("owner command requested review after confirmation")
    if "--plan" in argv:
        argv.remove("--plan")
    command = argv[:]
    while command:
        if command[0] in {"--owner", "--json"}:
            command = command[1:]
        elif command[0] in {"--agent", "--home", "--credential", "--session",
                            "--idempotency-key"} and len(command) >= 2:
            command = command[2:]
        else:
            break
    if (command[:1] == ["up"] or command[:2] == ["agent", "add"]) and "--name" not in argv:
        try:
            name = result["plan"]["clients"][0]["plan"]["spec"]["name"]
        except (KeyError, IndexError, TypeError) as error:
            raise ValueError("onboarding plan omitted its generated agent name") from error
        if not isinstance(name, str) or not name:
            raise ValueError("onboarding plan returned an invalid agent name")
        argv.extend(["--name", name])
    return [*argv, "--confirm", plan_id]
