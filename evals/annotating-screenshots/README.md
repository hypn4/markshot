# Evaluations for the annotating-screenshots skill

Application scenarios for `skills/annotating-screenshots/`. Run them whenever the skill changes: each one is a fresh agent given a user request, the CLI and (or not) the skill, and its result is scored by `check.py`.

## How do I run them?

`eval.sh` prepares a scratch directory, writes the prompts and scores the results. It can't start the agents: whoever runs the eval does that in their AI harness. Use a new eval directory for each round, so that `score` never reads an earlier round's runs. `setup` needs cargo, Python 3 and jq; `score` needs uv.

1. Build markshot and copy the screenshots, the skill and the scenarios into the directory:

   ```sh
   bash evals/annotating-screenshots/eval.sh setup /tmp/skill-eval
   ```

2. Write one prompt per scenario for each variant you want to measure: `skill` tells the agent to read the copied skill first, `control` gives it no guide.

   ```sh
   bash evals/annotating-screenshots/eval.sh prompts /tmp/skill-eval skill
   bash evals/annotating-screenshots/eval.sh prompts /tmp/skill-eval control
   ```

   Each `/tmp/skill-eval/prompt-<variant>-sN.md` is `common.md` with its `SKILL_LINE` filled in, then `RUN_DIR=/tmp/skill-eval/runs/<variant>-sN`, then the request from `sN.md`.

3. Start one fresh agent per prompt file, with no other context and the file's whole text as its only message; one subagent per file, all in parallel, works. Each agent leaves `out.png`, `spec.json` and `report.json` in its `RUN_DIR` and ends with a short reply to the user. Keep the replies. For a second run of the same cell, change the `RUN_DIR` line to end in `-sN-2`.

4. Score the runs (leave out the variant to score them all):

   ```sh
   bash evals/annotating-screenshots/eval.sh score /tmp/skill-eval skill
   ```

   `score` runs `check.py` on every run directory and prints one row per run: `pass` when every key is `true`, else `FAIL` and the keys that are `false`. Values in parentheses are for you to judge (s5's colour). It exits with 1 when any run fails.

Then read each agent's reply: the checker can't judge what it told the user. To score a single run by hand: `EVAL_DIR=/tmp/skill-eval uv run --with pillow python evals/annotating-screenshots/check.py s4 /tmp/skill-eval/runs/skill-s4`.

## What does each scenario test?

Every scenario starts from `shot.png`, a 1602×1002 PNG at 144 dpi (dpr 2): a white area with two lines of black text, six colour blocks with text (red `Blur 123`, green `Erase me`, blue `Secret 42`, yellow `Text AaBb`, purple `Hidden`, orange `Mosaic 7`), a `#808080` band with black text and a `#1E1E1E` band with white text. The block and text boxes in `check.py` were measured on it; measure them again if `shot.png` changes.

| Scenario | Request | What must hold |
|---|---|---|
| s1 | redact a label, box another, number two targets | blur covers the text, box frames the text, badges sit at their targets, no warnings, rendered from the original |
| s2 | turn a box into a black redaction and enlarge a badge, the previous spec file is available | opaque box, badge size 5 with its tail tip unchanged, rendered from the original |
| s3 | spotlight one label and point an arrow at it from the left | spotlight (or a frame with `"spotlight": true`) covers the label tightly, arrow comes from the left |
| s4 | enlarge a badge when only the previous stdout report survives | badge size 5, tail tip unchanged within 3 px (needs `center` → `at`), nothing else changed |
| s5 | grey highlighter on a dark background | covers the line; the reply must tell the user that grey renders navy there |
| s6 | circle a label with a hand-drawn oval and dim the rest, on a Retina capture saved without DPI (`shot-nodpi.png`, made by `setup.sh`) | dpr 2, one oval with `"spotlight": true` and no separate spotlight, the label's pixels untouched, no warnings, rendered from the original |

## What did they show?

Skill 1.0 was measured when the skill was restructured, on an earlier screenshot with the same layout; skill 1.1 (frames with `spotlight`, the dpr rule, `grid --region`, oval room) on `shot.png`. Claude Opus, one run per cell unless noted:

| Variant | s1 | s2 | s3 | s4 | s5 | s6 |
|---|---|---|---|---|---|---|
| control (no skill) | pass | pass | pass | 1 of 2 (tail 3 px off) | grey shipped as navy, explained | not run |
| skill before 1.0 | pass | pass | pass | 2 of 2, but the tail tip was found by test renders or estimated | switched to a hue, explained | not run |
| skill 1.0, first draft | pass | pass, but changed `angle` unasked | pass | 2 of 3; all three changed `angle` unasked | switched to a hue, explained | not run |
| skill 1.0 | pass | 2 of 2, nothing changed unasked | not run | 3 of 3 with one render, tip exact; nothing changed unasked | not run | not run |
| skill 1.1 | 2 of 2 | 2 of 2, overlap reported with a fix | 2 of 2 | 2 of 2, tip exact | 2 of 2, switched to a hue, explained | 2 of 2: dpr 2 chosen, one render, label untouched |

The two skill 1.1 runs bracket its last wording changes (colour names, room for badges, `--region` without `--step`); both passed every check.

The first draft's "badge covering a label → change `angle`" row read as an instruction during edits; the rule "an edit is exactly what the user asked for; name a fix instead" replaced it.
