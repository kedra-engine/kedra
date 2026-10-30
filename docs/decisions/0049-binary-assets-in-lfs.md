# ADR 0049: Binary source assets in Git LFS

Status: accepted by the repository owner.

## Decision

Store binary source assets in Git LFS, including small images, fonts and model
fixtures. `.gitattributes` declares the binary formats; text sources, JSON glTF,
scene/prefab documents and `.meta` sidecars remain ordinary Git content. Asset
UUIDs and licensing/provenance records are independent of LFS object identities.

Git LFS is the maintained storage/transport mechanism. Do not split binary files,
encode them as text, exempt asset directories or rewrite repository history to
fit the authored-source size gate. Existing historical Git blobs remain intact;
subsequent approved commits store matching assets as LFS pointers.

The file gate requires LFS attributes for binary files and hydrated working-tree
inputs. Binary LFS objects are not subject to the 64 KiB authored-file budget.
UTF-8 authored files retain the 1000-line / 64 KiB limits even if an LFS pattern
matches them. Existing exact generated/license exceptions remain unchanged.
Runtime/source parser budgets and delivery limits are separate contracts.

## Checkout and publication

Install Git LFS before cloning, or run `git lfs install` and `git lfs pull` in an
existing checkout. CI checkouts use `lfs: true`; builds/packages consume hydrated
bytes, never pointers. Ordinary Git source archives may omit LFS content and are
not a substitute for a hydrated checkout.

For conversion of existing tracked assets, stage only matching binary paths with
`git add --renormalize -- <paths>`. Inspect staged pointer diffs and run
`git lfs fsck --objects` before an approved commit. Normal `git push` uses the
LFS pre-push hook to transfer referenced objects; it still requires the owner's
publication authorization. Do not use `git lfs migrate` to rewrite history.

## Limits

LFS server availability, authentication, quotas and remote object upload require
hosted verification. Local installation and clean/smudge checks do not prove that
objects have been published. LFS does not change third-party license obligations
or certify animation/material support in the runtime.
