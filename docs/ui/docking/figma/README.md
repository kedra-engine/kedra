# Design provenance only

`manifest.json` preserves the original Figma file/node identities and hashes used
when documenting the dock skin's source. The former full-window PNG corpus and
its pixel policies are retired. This metadata is not active screenshot coverage
and does not enroll docking, panels or window chrome in visual regression.

Only kit buttons and text inputs have screenshot coverage; see
[the active corpus](../../../site/en/contributing/contracts/ui/ui-corpus.md). Docking uses
[native behavior checks](../../../site/en/contributing/contracts/ui/native-docking-tests.md),
without image goldens.
