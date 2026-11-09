# cigcompile guides

Start at the top if you're new; jump to the reference when you need a flag
or an error code.

**Learn it**

- [Getting started](getting-started.md): install cigcompile, build your
  first program, run it on a machine with no cig, then build one for
  another machine and verify what came out.

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
- [Roadmap](roadmap.md): the stages, the phases of #42, and how a request
  gets there.

Stuck on something these don't cover? [Ask for
guidance](https://github.com/otmof-ops/cig-compile/issues/new?template=guidance-request.yml);
a person answers it.
