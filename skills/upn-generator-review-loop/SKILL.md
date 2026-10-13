---
name: upn-generator-review-loop
description: Repeat independent Codex review and repairs on a UPN Generator branch or PR with one GPT-6.1-Sol Medium read-only session until no findings remain. Use for user-requested review/fix loops in this repository; do not start it as an automatic implementation gate.
---

# UPN Generator Review Loop

Use this skill only when the user asks for a review loop, a Codex review with fixes, or "review until clean" on this repository. It composes the machine-wide `codex-review` skill (invocation, session reuse, resume, Codex homes) with this repository's own review standard, verification, and delivery rules from `AGENTS.md`. The loop never merges, never bumps the version, and never tags; releases are the user's separate step.

## Procedure

1. **Resolve the target.** Use the PR from the user request or the current branch; otherwise review the branch diff against `origin/main`. Record the checkout or worktree path, base SHA, and head SHA. Register the PR with the host when supported. PR creation belongs to the user's separate request. Start with `git status --short` and `git branch --show-current`; unrelated dirty files are preserved and excluded from the review target.
2. **Start one reviewer session** following `codex-review`: `gpt-6.1-sol`, `medium` reasoning, read-only sandbox, run from a non-repo directory, stdin closed, no `--ephemeral`. Reuse this session for every round of this loop. The prompt must give the reviewer:
   - the absolute checkout/worktree path, PR URL when one exists, and base/head SHAs;
   - the `Code Review Rules` section of `AGENTS.md` as the finding standard (concrete defects only, severity, `file:line`, failure scenario, most severe first);
   - the `Architecture decisions` and `Implementation contracts` sections of `AGENTS.md` as the correctness authority;
   - the repository's risk areas to inspect explicitly: Credential Manager and backup handling, closed-month gates, IPC registration/`ipc.ts`/`types.ts` sync, migrations with backup/restore and factory reset, `WorkflowSnapshot` state ownership, and UPN form fidelity against `file-examples/`;
   - verification evidence already gathered and the manual QA checklist files touched;
   - an instruction to review independently, make no edits, and return "no findings" plainly when nothing meets the bar.
3. **Claim-check every finding** against the current tree before acting. Confirm the defect from source; mark stale or incorrect premises **Possibly stale/incorrect** with the reason. Never change code solely because a finding was reported. Return disputed findings with evidence instead of silently dropping them.
4. **Repair confirmed findings** at the owning command, migration, type, or component, not the symptom. Then run the verification `AGENTS.md` requires for the changed boundary (`cargo check`, relevant `cargo test`, `npm.cmd run build`), reset affected manual QA cases to unchecked, update affected docs, and add a lesson to `.agents/lessons.md` only when it is generalizable. Commit per `Git and delivery` and push when the branch has an open PR. Keep Git writes separate from implementation edits.
5. **Re-review in the same session** by resuming it with the new head SHA, the commit list, and the repair evidence. Require re-review of prior findings and the affected changes only; a follow-up review must not replace a fixed finding with an adjacent hypothetical.
6. **Repeat steps 3 to 5** until the reviewer reports no remaining findings on the current head. If successive fixes require increasingly artificial special cases, or one finding conflicts with an established architecture decision, stop patching and reassess the owning boundary with the user instead of chasing convergence.
7. **Report.** State the PR, every commit in the `review <path>` / `Commit:` block form, verification performed and outstanding (including manual QA cases left unchecked), disputed findings with evidence, the `Docs updated` / `Docs audit` line, and the clean verdict. If blocked by missing evidence or a required user decision, report the blocker without claiming convergence. Do not merge.

## Presentation

Render findings as prose, not pasted reviewer output: severity and `file:line` first, then the defect and failure scenario. Claude follows the stricter rule in the `# Claude Instructions` section of `CLAUDE.md`.
