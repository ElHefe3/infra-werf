# D024 — Windows UI helper returns a mismatched screenshot for Atlas

Date: 2026-10-26 · T013/T019. State: **resolved for rebuilt-app inspection; T019's native health journey remains open**.

## Finding

The computer-use helper's accessibility tree bound to the exact running `atlas-native.exe` window titled `Atlas` and returned Atlas's current WebView content. An accessibility click failed with `coordinate input geometry is unavailable` and did not execute. The subsequent screenshot showed the ChatGPT window instead of Atlas. Refreshing the Windows window inventory and rebinding the same Atlas window ID returned the correct accessibility tree but another ChatGPT screenshot.

## Safe response

Per the computer-use recovery rule, no further click or coordinate action was attempted after the single target refresh. The Atlas app remains open and is the older bundle, so rebuilding the locked executable also requires a normal Atlas close. Do not force-stop the process or use coordinates from the unrelated screenshot.

## Follow-up

The owner closed the old process normally. The rebuilt app launched, returned its own Atlas accessibility tree and screenshot, and was inspected at desktop size. The T013 controlled native journey then passed in the rebuilt Tauri app. T019's health transition still needs its own disposable native journey. See [T013 progress](../evidence/T013-progress.md), [T019 progress](../evidence/T019-progress.md) and [the checkpoint](../evidence/CHECKPOINT.md).
