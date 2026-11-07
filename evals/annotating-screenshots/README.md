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

   `score` runs `check.py` on every run directory and prints one row per run: `pass` when every key is `true`, else `FAIL` and the keys that are `false`. Values in parentheses are for you to judge (s5's colour, s7's toggle frames). It exits with 1 when any run fails.

Then read each agent's reply: the checker can't judge what it told the user. To score a single run by hand: `EVAL_DIR=/tmp/skill-eval uv run --with pillow python evals/annotating-screenshots/check.py s4 /tmp/skill-eval/runs/skill-s4`.

## What does each scenario test?

Scenarios s1 to s6 start from `shot.png`, a 1602×1002 PNG at 144 dpi (dpr 2): a white area with two lines of black text, six colour blocks with text (red `Blur 123`, green `Erase me`, blue `Secret 42`, yellow `Text AaBb`, purple `Hidden`, orange `Mosaic 7`), a `#808080` band with black text and a `#1E1E1E` band with white text. The block and text boxes in `check.py` were measured on it; measure them again if `shot.png` changes.

s7 and s8 start from `console.png`, a 2800×1800 Retina capture of a made-up cloud console's settings page, saved without DPI as many real captures are: a left nav, tabs, a dense four-row Services table whose status pills leave less room on every side than a default badge needs, and a Sharing policy panel with segmented button groups and toggles whose one-line grey descriptions sit 9 CSS px below them. It is `console.html` rendered by headless Chrome at device scale factor 2. To regenerate it, from the repository root:

```sh
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 --window-size=1400,900 --screenshot="$PWD/evals/annotating-screenshots/console.png" "file://$PWD/evals/annotating-screenshots/console.html"
```

Chrome writes the PNG without a pHYs chunk, which the scenarios need; check that it still does. Another Chrome version or set of system fonts moves the text, so measure the boxes in `check.py` again with `grid --region` after regenerating it.

| Scenario | Request | What must hold |
|---|---|---|
| s1 | redact a label, box another, number two targets | blur covers the text, box frames the text, badges sit at their targets, no warnings, rendered from the original |
| s2 | turn a box into a black redaction and enlarge a badge, the previous spec file is available | opaque box, badge size 5 with its tail tip unchanged, rendered from the original |
| s3 | spotlight one label and point an arrow at it from the left | spotlight (or a frame with `"spotlight": true`) covers the label tightly, arrow comes from the left |
| s4 | enlarge a badge when only the previous stdout report survives | badge size 5, tail tip unchanged within 3 px (needs `center` → `at`), nothing else changed |
| s5 | grey highlighter on a dark background | covers the line; the reply must tell the user that grey renders navy there |
| s6 | circle a label with a hand-drawn oval and dim the rest, on a Retina capture saved without DPI (`shot-nodpi.png`, made by `setup.sh`) | dpr 2, one oval with `"spotlight": true` and no separate spotlight, the label's pixels untouched, no warnings, rendered from the original |
| s7 | on `console.png`: box the button group under a named setting, circle two adjacent toggles with their labels by hand, number the three and dim the rest | dpr 2; each target in a frame of its own with `"spotlight": true` and no separate spotlight; badges 1 to 3 at their frames; the targets' text and switches untouched; no frame line on the descriptions and rows next to them; no warnings; rendered from the original. `toggle_frames` names the toggles' frames for you to judge: the user asked for ovals |
| s8 | on `console.png`: number the four status cells of a dense table | dpr 2; badges 1 to 4, each marking its own row's status pill; no badge circle on a pill or on any other text in the table; no warnings; rendered from the original |

## What did they show?

Skill 1.0 was measured when the skill was restructured, on an earlier screenshot with the same layout; skills 1.1 to 1.5 use `shot.png` and `console.png`. Skill 1.1 added frames with `spotlight`, the dpr rule, `grid --region` and oval room guidance; 1.2 added `crop`, `"angle": "auto"`, `text` and dense-row guidance; 1.3 added `around`; 1.4 added an optional white `ring` on counters; 1.5 lets markshot pick an oval's line thickness when `size` is omitted and documents no-fit warnings. No skill 1.4 run needed a ring, so that row checks that the new option changes nothing else. Earlier rows used Claude Opus; skill 1.5 used fresh agents in the current harness. One run per cell unless noted:

| Variant | s1 | s2 | s3 | s4 | s5 | s6 | s7 | s8 |
|---|---|---|---|---|---|---|---|---|
| control (no skill) | pass | pass | pass | 1 of 2 (tail 3 px off) | grey shipped as navy, explained | not run | not run | not run |
| skill before 1.0 | pass | pass | pass | 2 of 2, but the tail tip was found by test renders or estimated | switched to a hue, explained | not run | not run | not run |
| skill 1.0, first draft | pass | pass, but changed `angle` unasked | pass | 2 of 3; all three changed `angle` unasked | switched to a hue, explained | not run | not run | not run |
| skill 1.0 | pass | 2 of 2, nothing changed unasked | not run | 3 of 3 with one render, tip exact; nothing changed unasked | not run | not run | not run | not run |
| skill 1.1 | 2 of 2 | 2 of 2, overlap reported with a fix | 2 of 2 | 2 of 2, tip exact | 2 of 2, switched to a hue, explained | 2 of 2: dpr 2 chosen, one render, label untouched | not run | not run |
| skill 1.2 | pass | pass, overlap reported with a fix | pass | pass, tip exact; reported that no angle clears the label | pass, switched to a hue, explained | pass | 0 of 7 (see below) | pass: size 1 badges after "auto" crowded the rows |
| skill 1.3 (CLI 0.2.0) | pass | pass, overlap reported with a fix | pass | pass, tip exact; suggested `"auto"` or size 4 | pass, switched to a hue, explained | pass, with `around` | 3 of 3, with `around`; ovals kept clear of target and text | pass: size 0 badges |
| skill 1.4 (CLI 0.3.0) | pass | pass, overlap reported with a fix | pass | pass, tip exact; tried `"auto"` and another angle as fixes | pass, switched to a hue, explained | pass, with `around` | 3 of 3, with `around` | pass: size 1 badges |
| skill 1.5 (CLI 0.4.0) | FAIL: checker requires blur; used an opaque black box | pass, overlap reported with a fix | pass | pass, tip exact; overlap reported with a fix | pass, switched to yellow, explained | pass, automatic oval thickness | 3 of 3, automatic thickness from size 0 to 2 | pass: size 0 badges |

The two skill 1.1 runs bracket its last wording changes (colour names, room for badges, `--region` without `--step`); both passed every check.

s7 was the hard case: hand-drawn ovals around toggles whose description lines sit 9 CSS px away. Under skill 1.2 all seven runs failed, first by crossing a description line and then, with written room tables, by touching the toggle's own box by 1 to 2 px; sizing a wobbly spiral by hand was too fiddly. CLI 0.2.0 added `around`: the agent gives the target's box and markshot sizes the oval against the image's own ink, picking a seed whose wobble fits when the spec leaves it out. With skill 1.3 all three runs passed with one or two renders, letting the ovals run past the panel's edge rather than onto the text.

For skill 1.5, all three s7 runs passed with no warnings; the final resolved specs recorded sizes from 0 to 2 and re-rendered identically. The incomplete earlier attempts had left only inspection previews; fresh agents reused those previews and produced the final specs, images and reports.

The skill 1.5 s1 run failed `blur_covers_secret`: the agent chose a solid black `rect`, while the checker accepts only `blur`. All 4104 pixels of the measured secret-text region were opaque black, so the target was fully redacted. The checker was not changed to turn this into a pass; the other nine scored runs passed.

The two skill 1.1 runs bracket its last wording changes (colour names, room for badges, `--region` without `--step`); both passed every check.

The first draft's "badge covering a label → change `angle`" row read as an instruction during edits; the rule "an edit is exactly what the user asked for; name a fix instead" replaced it.
