# Publishing the public repository

Owner decision [D062](discovery/D062-licence-and-public-repository.md). Publishing is the owner's action. Nothing here pushes anything; every push below is typed by the owner.

## What stays private

- The private repository on the owner's Forgejo, with its full, unrewritten history. It is never pushed to the public remote.
- `.public-terms.json` at the repository root: the owner's server names, account, addresses, paths and other terms, each with its placeholder. It is ignored by Git and never leaves this PC. Add to it whenever a new private term appears.

## Each publication

1. Finish and merge the work to `master` in the private repository as usual.
2. Check the tree: `python -I scripts/public_repo.py check`. It must report 0 problems. Fix any with `python -I scripts/public_repo.py scrub` (then review and commit) or by hand.
3. Make the public copy in a new folder outside the repository:

   ```powershell
   python -I scripts/public_repo.py copy --dest ..\infrawerf-public --ref master
   ```

   It rewrites every commit of `master` with the owner's replacements into a new repository whose branch is `main`, then checks every file version and commit message in it. It must report 0 problems.
4. Build the copy from its own documents alone, in a clean clone of it: `mise trust`, `mise install`, `mise run deps`, `mise run check`, `mise run smoke:desktop`.
5. Read the copy's `README.md`, `CONTRIBUTING.md`, `SECURITY.md` and `docs/COMPATIBILITY.md` once more as a stranger would.
6. On GitHub, create an empty public repository. Under **Settings → Code security**, turn on **Private vulnerability reporting**, which `SECURITY.md` relies on.
7. From the copy, add the GitHub repository as its only remote and push `main`. Do not add the public remote to the private repository.

Later publications repeat steps 1–5 into a new folder. The rewrite is deterministic for the same terms, so unchanged commits keep the same public hashes and the push is a fast-forward; if the terms change, earlier public hashes change too and the push needs the owner's decision to replace the public history.

## Notes

- Commit hashes cited in evidence refer to the private history.
- Authors appear as their GitHub no-reply addresses, as in the private history.
