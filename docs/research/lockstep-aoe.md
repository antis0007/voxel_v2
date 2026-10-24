# Lockstep networking (1500 Archers on a 28.8)
Source: Paul Bettner and Mark Terrano, "1500 Archers on a 28.8: Network Programming in Age of Empires and Beyond", GDC 2001 (https://www.gamedeveloper.com/programming/1500-archers-on-a-28-8-network-programming-in-age-of-empires-and-beyond, read via article summary).
Adopt: Send only player commands, run the identical deterministic sim on every machine, schedule commands two turns ahead; checksum world, pathing and targeting state; keep RNG call counts identical; use the desync-trace approach.
Reject: Sending per-unit state (paper: caps at about 250 moving units); the 1990s UDP resend scheme and 8-player 28.8 targets.
Why: Command-only lockstep suits our integer-only cellular and factory state, but any float or thread-order dependence in the tick would break it.
