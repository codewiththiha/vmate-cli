# Resource measurements

No before/after RSS or throughput measurements have been collected.
Do not label these changes a measured performance improvement.

Structural bounds: output channels have 512 entries, concurrent probes use a
semaphore, completed scan tasks are drained during scheduling, and the PID
registry removes normally torn-down children. Log line byte sizes and discovered
path counts are not capped, so these are not absolute memory bounds.

For a benchmark record OS/kernel/CPU/RAM, OpenVPN version, config count,
transport, workers, timeout, success ratio and command. Compare peak RSS and
elapsed time with `/usr/bin/time -v` on Linux and `/usr/bin/time -l` on macOS.
Use the same controlled local VPN server, not changing public relays.
Measure fd count and surviving child processes after cancellation as well.
