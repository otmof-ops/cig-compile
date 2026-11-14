#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
# SPDX-License-Identifier: FSL-1.1-ALv2
"""memsplit.py [--no-thp] -- CMD...: run CMD once, stopped at its exit while its address space is
still there, and print what it held: its peak (VmHWM), what is its own, the pages of files
(RssFile + RssShmem), and how much of each the kernel had in 2 MiB pages (smaps_rollup). --no-thp
turns transparent huge pages off for the program before it starts (prctl PR_SET_THP_DISABLE, kept
across exec), so the figures are what it touched, to the 4 KiB page."""
import ctypes, os, signal, sys

libc = ctypes.CDLL(None, use_errno=True)
libc.ptrace.argtypes = [ctypes.c_long, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p]
libc.ptrace.restype = ctypes.c_long
TRACEME, CONT, SETOPTIONS = 0, 7, 0x4200
O_TRACEEXIT, O_EXITKILL, EVENT_EXIT = 0x40, 0x100000, 6


def kb(text, key):
    for line in text.splitlines():
        if line.startswith(key + ":"):
            return int(line.split()[1])
    return 0


def main():
    args = sys.argv[1:]
    no_thp = args[0] == "--no-thp"
    if no_thp:
        args = args[1:]
    assert args[0] == "--", "usage: memsplit.py [--no-thp] -- CMD..."
    argv = args[1:]
    pid = os.fork()
    if pid == 0:
        if no_thp and libc.prctl(41, 1, 0, 0, 0) != 0:
            os._exit(126)
        libc.ptrace(TRACEME, 0, None, None)
        null = os.open(os.devnull, os.O_RDWR)
        os.dup2(null, 0)
        os.dup2(null, 1)
        os.execv(argv[0], argv)
    os.waitpid(pid, 0)  # the stop its exec makes
    libc.ptrace(SETOPTIONS, pid, None, ctypes.c_void_p(O_TRACEEXIT | O_EXITKILL))
    sig = 0
    while True:
        libc.ptrace(CONT, pid, None, ctypes.c_void_p(sig))
        _, st = os.waitpid(pid, 0)
        if os.WIFEXITED(st) or os.WIFSIGNALED(st):
            print("ended before it could be read", st)
            return 1
        if st >> 8 == (signal.SIGTRAP | (EVENT_EXIT << 8)):
            status = open(f"/proc/{pid}/status").read()
            roll = open(f"/proc/{pid}/smaps_rollup").read()
            libc.ptrace(CONT, pid, None, None)
            _, st = os.waitpid(pid, 0)
            mb = lambda k: f"{k / 1024:6.1f}"
            files = kb(status, "RssFile") + kb(status, "RssShmem")
            # its own at the peak: what it held at the end, or the peak less its files when that is more
            own = max(kb(status, "RssAnon"), kb(status, "VmHWM") - files)
            print(
                "peak", mb(kb(status, "VmHWM")),
                "own", mb(own),
                "files", mb(files),
                "| in 2 MiB pages: own", mb(kb(roll, "AnonHugePages")),
                "files", mb(kb(roll, "FilePmdMapped") + kb(roll, "ShmemPmdMapped")),
                "| exit", os.WEXITSTATUS(st) if os.WIFEXITED(st) else st,
            )
            return 0
        s = os.WSTOPSIG(st)
        sig = 0 if s == signal.SIGTRAP else s


sys.exit(main())
