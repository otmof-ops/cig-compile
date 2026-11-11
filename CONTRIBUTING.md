# Contributing

The plan of record is the issues: each says why, what lands, and when it is
done. Pick one, or open one for what you have in mind before writing much.

## Building and testing

```sh
cargo build --release          # target/release/cigcompile and target/release/cig-runtime
cargo test                     # unit and end-to-end tests
CIGSCRIPT_SRC=../cig-script cargo test --release --test differential   # the differential suite
```

To use a checkout's build in place of an installed cigcompile, run
`target/release/cigcompile`; it finds the `cig-runtime` beside it first.
People who are not developing cigcompile install it with
[ciginstaller](https://github.com/otmof-ops/cig-installer#install).

The differential suite needs a checkout of CigScript at the version this
compiler pins (`Cargo.toml`). CI runs formatting, clippy, the tests, the
differential suite and the budgets on five machines. A program that says or
leaves anything other than what its script does under `cig run` fails the
build.

A generated program that differs (`DIFF_RANDOM`, see the README) prints its
seed and its smallest form: `DIFF_RANDOM=<seed>:1 DIFF_FILTER=random/` replays
it, and the smallest form belongs in the pull request that fixes it.

## Pull requests

One subject per pull request, naming its issue (`Closes #9`), with the test
that proves it. Reviews are about the code and the behavior; expect a reply,
and expect it to be kind.

## Licensing your contribution

Code is FSL-1.1-ALv2 and documentation is CC BY 4.0; [REUSE.toml](REUSE.toml)
maps every path, and source files carry SPDX headers. A new source file gets
the same two lines as its neighbors, with the code identifier:

```text
SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
SPDX-License-Identifier: FSL-1.1-ALv2
```

Documentation files use `CC-BY-4.0` in the second line. See [LICENSING.md](LICENSING.md)
for which versions carry which license.

## The contributor agreement

CigCompile uses a Contributor License Agreement, not a DCO. Before a pull
request from you can be merged, you sign the agreement once:

- individuals: [`.github/CLA/ICLA.md`](.github/CLA/ICLA.md)
- companies whose employees contribute: [`.github/CLA/CCLA.md`](.github/CLA/CCLA.md)

You keep your copyright. The agreement gives the project a license to use,
relicense and distribute your contribution, a patent license for what you
contribute, and your confirmation that the work is yours to give. It exists so
the project can, for example, offer a commercially licensed edition or move to
a newer license later without tracking down every past contributor. Signing is
a comment on your first pull request:

> I have read the CLA Document and I hereby sign the CLA

A bot records it; you will not be asked again.
