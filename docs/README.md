# cigcompile guides

Start at the top if you're new; jump to the reference when you need a flag
or an error code.

**Learn it**

- [Getting started](getting-started.md): install cigcompile with
  ciginstaller, build your first program, run it on a machine with no
  cig, then build one for another machine and verify what came out.
  Updating and removing cigcompile are ciginstaller's too:
  `ciginstaller update` and `ciginstaller uninstall cigcompile`.

**Look it up**

- [Commands](commands.md): every command and flag, the exit codes, the
  error codes, the environment variables, and the files cigcompile keeps.
- [Benchmarks](BENCHMARKS.md): the benchmark set, how a run is measured,
  and where stage A sits.

**Make it yours**

- [Shipping a program](shipping.md): signatures, SBOMs, the hardening
  report, OCI images, `.deb` packages, Homebrew and Scoop manifests,
  reproducible builds, and the license that travels with a program.
- [Troubleshooting](troubleshooting.md): what a message means and what to
  type next, keyed by the error code and the line a user sees.

**Under the hood**

- [How it works](how-it-works.md): the compiler's insides, for
  contributors.
- [The backend decision](decisions/0001-backend.md): why stage A exists,
  and what stages B to D are for.
- [Payload format 2](decisions/0004-payload-format-2.md): the container a
  program's bytecode, archives, own flags and provenance go in, and the
  order they land in.
- [What goes native first](decisions/0005-what-goes-native-first.md): how
  stages C and D start, as typed specializations compiled ahead of time.
- [Native code in a program](decisions/0006-native-code-in-a-program.md):
  the section, the loader, the calling convention, and leaving native code.
- [Where the pieces live](decisions/0007-where-the-pieces-live.md): the
  crates of the inference, the code generator and the loader.
- [Native code on the VM's frame](decisions/0008-native-code-on-the-vms-frame.md):
  native code for every function, run on the frame the VM made, with the
  bytecode going on wherever it stops.
- [Research notes](research/README.md): 17 studies of other compilers,
  and what cig's own VM was measured doing.
- [Roadmap](roadmap.md): the stages, the phases of #42, and how a request
  gets there.

Stuck on something these don't cover? [Ask for
guidance](https://github.com/otmof-ops/cig-compile/issues/new?template=guidance-request.yml);
a person answers it.
