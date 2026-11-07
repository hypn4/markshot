# Evaluations for the annotating-screenshots skill

Application scenarios for `skills/annotating-screenshots/`. Run them whenever the skill changes: each one is a fresh agent given a user request, the CLI and (or not) the skill, and its result is scored by `check.py`.

## How do I run them?

```sh
bash evals/annotating-screenshots/setup.sh /tmp/skill-eval
```

For each scenario `sN`, start a fresh agent with no other context and give it `common.md` + `sN.md` from the eval directory, with `RUN_DIR=/tmp/skill-eval/runs/<variant>-sN` and `SKILL_LINE` replaced by either:

- skill: `Before starting, read the agent skill /tmp/skill-eval/skill/SKILL.md and follow it.`
- control: `No usage guide is provided; use the CLI's --help and schema.`

Then score each run:

```sh
EVAL_DIR=/tmp/skill-eval uv run --with pillow python evals/annotating-screenshots/check.py s4 /tmp/skill-eval/runs/skill-s4
```

Every key must be `true`. Also read the agent's reply: the checker can't judge what it told the user.

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
