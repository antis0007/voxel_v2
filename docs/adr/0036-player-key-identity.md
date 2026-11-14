# ADR 0036: Player key identity

2026-10-27. Accepted for multiplayer hosts with access lists.

Operator rights, the whitelist and bans were keyed by the name a player typed.
A name is self-chosen, so anyone could join under an offline operator's name
and receive that operator's rights. The interim profile token fixed the name
binding but travelled as plain application data, readable by the relay and
replayable by anyone who saw it.

Each client now keeps one Ed25519 key (`engine_net::identity::PlayerKey`). The
host's `Welcome` carries 32 random bytes per connection. The client's
`Introduce` carries its public key and a signature over a domain tag, that
challenge and the world identity, so a captured signature is useless on any
other connection or world. The host verifies it before admitting the name, and
the key cannot change for the rest of the connection. Rights follow the key's
fingerprint (the first 16 bytes of the public key, as 32 hex digits): an
operator entry `op=name|<fingerprint>` grants nothing until bound, and a name
bound to another key is refused. A whitelisted name is claimed by the first key
that proves it (`white=name|<fingerprint>`) and refuses every other key; with
the whitelist on, a peer that proved no key is refused. The whitelist and bans
also accept bare fingerprints (`white-key=`, `ban-key=`). Protocol `VERSION` is 5.

Steam login was rejected for now: it needs a Steamworks App ID, players
launching through Steam, and a binary SDK, and it covers only Steam
connections while the relay is the main path. A verified Steam ID can later map
to the same fingerprint slot.

Dependencies: `ed25519-dalek` 2.1 for signatures; `getrandom` 0.3 for keys and
challenges from the operating system's secure generator. Simulation randomness
stays positional hashing; these bytes never reach game state.

Not covered: the relay link is unencrypted, so a malicious host or relay that
proxies a whole connection can still read and forward traffic. Channel
encryption bound to these keys is the follow-up. A lost key file means the host
re-binds that operator by hand.
