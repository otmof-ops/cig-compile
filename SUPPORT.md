# Getting help

I read every request. Put yours where it fits and you'll get an answer
in the issue.

| you want to | open |
|---|---|
| get unstuck compiling CigScript to programs | [a guidance request](https://github.com/otmof-ops/cig-compile/issues/new?template=guidance-request.yml) |
| have cigcompile build for a machine or write an output it doesn't | [a target request](https://github.com/otmof-ops/cig-compile/issues/new?template=target-request.yml) |
| get something another compiler or tool does better | [a quality-of-life request](https://github.com/otmof-ops/cig-compile/issues/new?template=qol-request.yml) |
| report something that went wrong | `ciginstaller bug cigcompile` once your installer has it; until then, [a bug report](https://github.com/otmof-ops/cig-compile/issues/new?template=bug-report.yml) |
| report a problem in the language itself | [CigScript's issues](https://github.com/otmof-ops/cig-script/issues) |
| report a security problem | [SECURITY.md](SECURITY.md), never a public issue |

## What helps

- `cigcompile --version` and `cig --version`, and your platform.
  `ciginstaller status` says what is installed, and `ciginstaller
  doctor --tools` checks it, with cigcompile's own view of its runtime
  and caches.
- The script, or the smallest one that still shows the problem.
  `cigcompile reduce <script>` cuts one down for you.
- The command you ran and everything it printed. Paste, don't
  summarize.
- For a program whose behavior is wrong, `cigcompile verify <program>
  --script <script>`: it runs nothing and names the first part that
  differs.

## Installing, updating, removing

cigcompile is installed by [ciginstaller](https://github.com/otmof-ops/cig-installer#install).
`ciginstaller update` brings it current, and
`ciginstaller uninstall cigcompile` removes it; neither is a cigcompile
command.

## What happens next

- **Guidance:** a direct answer, with the steps for your project, in
  the issue. If the answer should be in the docs, it goes in the docs
  too.
- **Bugs:** a fix, and a test that keeps it fixed. The issue is
  closed with the version that has it.
- **Target and quality-of-life requests:** a reply saying what I
  think and why. Accepted ones get a line on the
  [roadmap](docs/roadmap.md) and are built with you.

## Why this page exists

cigcompile exists so CigScript scripts can be run on a machine with
nothing installed, and a tool like that is only as good as the help
behind it. If something here is confusing, that's a bug in the docs,
and I'd rather hear about it than have you give up.
