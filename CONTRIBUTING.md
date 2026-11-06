# Contributing

The plan of record is the issues: each says why, what lands, and when it is
done. Pick one, or open one for what you have in mind before writing much.

## Building and testing

```sh
cargo build --release          # target/release/cigcompile and target/release/cig-runtime
cargo test                     # unit and end-to-end tests
CIGSCRIPT_SRC=../cig-script cargo test --release --test differential   # the differential suite
```

The differential suite needs a checkout of CigScript at the version this
compiler pins (`Cargo.toml`). CI runs formatting, clippy, the tests, the
differential suite and the budgets on five machines. A program that says or
leaves anything other than what its script does under `cig run` fails the
build.

## Pull requests

One subject per pull request, naming its issue (`Closes #9`), with the test
that proves it. Reviews are about the code and the behavior; expect a reply,
and expect it to be kind.

## Licensing your contribution

Code is Apache-2.0 and documentation is CC BY 4.0; [REUSE.toml](REUSE.toml)
maps every path, and every source file carries SPDX headers (a new one gets
the same two lines; copy them from a neighbor). A contribution is accepted
under the license of the kind of file it changes, which Apache-2.0 says itself
in its section 5. Sign off each commit (`git commit -s`): the `Signed-off-by`
line is your statement, under the
[Developer Certificate of Origin](https://developercertificate.org), that the
work is yours to give. You keep your copyright.
